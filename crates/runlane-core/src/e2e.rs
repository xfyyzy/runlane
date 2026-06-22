use std::path::Path;

use crate::{
    ActionKind, ActionTarget, AgentResultStatus, AgentResultSubmission, AgentTaskEnvelope,
    AuditEvent, AuditEventKind, AuditLedger, Capability, CapabilityReport, EvidenceEnvelope,
    HelperActionRequest, HelperActionResponse, HelperActionStatus, HelperAllowlist,
    HelperAllowlistEntry, HelperArgument, HelperValidationContext, LeaseMode, LeaseSignatureStatus,
    SignedCapabilityLease, VerificationPlanner, VerificationPolicy,
    analyzer::{ProposalPolicy, analyze_service_unhealthy, validate_proposal},
    approval::{ApprovalRecord, ApprovalStore},
    fleet::FleetRepository,
    receipt::{OperatorReceipt, generate_operator_receipt},
    runtime::{
        AgentEnrollmentRequest, ControlPlane, EnrollmentToken, PendingAgentTask, TypedTaskPayload,
        runtime_text_evidence,
    },
    validate_helper_request,
};

/// Ordered E2E journey stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JourneyStage {
    Declare,
    Observe,
    Propose,
    Approve,
    Lease,
    Execute,
    Verify,
    Remember,
}

/// Deterministic E2E simulation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceUnhealthySimulation {
    pub run_id: String,
    pub stages: Vec<JourneyStage>,
    pub receipt: OperatorReceipt,
    pub ledger: AuditLedger,
}

/// E2E simulation failure.
#[derive(Debug)]
pub enum E2eError {
    Fleet(String),
    Planning(String),
    Proposal(String),
    Approval(String),
    Helper(String),
    Receipt(String),
}

impl std::fmt::Display for E2eError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fleet(message)
            | Self::Planning(message)
            | Self::Proposal(message)
            | Self::Approval(message)
            | Self::Helper(message)
            | Self::Receipt(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for E2eError {}

/// Runs the deterministic service-unhealthy journey against a fleet fixture.
pub fn run_service_unhealthy_simulation(
    fleet_path: impl AsRef<Path>,
) -> Result<ServiceUnhealthySimulation, E2eError> {
    let run_id = "run-demo-service-unhealthy";
    let incident_id = "incident-demo-service-unhealthy";
    let fleet =
        FleetRepository::load(fleet_path).map_err(|error| E2eError::Fleet(error.to_string()))?;
    let mut ledger = AuditLedger::empty();
    append(
        &mut ledger,
        run_id,
        AuditEventKind::IncidentCreated {
            incident_id: incident_id.to_owned(),
            node_id: "prod-web-01".to_owned(),
            runbook: "service-unhealthy".to_owned(),
        },
    )?;

    let mut control_plane = ControlPlane::empty();
    for node in &fleet.inventory {
        control_plane
            .create_enrollment_token(EnrollmentToken::new(
                format!("token-{}", node.id),
                format!("enroll-{}", node.id),
                node.id.clone(),
                node.os,
                "demo-trust-root",
                200,
                format!("enroll-nonce-{}", node.id),
            ))
            .map_err(|error| E2eError::Planning(format!("{error:?}")))?;
        control_plane
            .enroll_agent(&AgentEnrollmentRequest::new(
                format!("enroll-{}", node.id),
                node.id.clone(),
                node.os,
                format!("cert-{}", node.id),
                "demo-trust-root",
                100,
            ))
            .map_err(|error| E2eError::Planning(format!("{error:?}")))?;
    }

    let target = fleet
        .inventory
        .iter()
        .find(|node| node.id == "prod-web-01")
        .ok_or_else(|| E2eError::Fleet("examples/fleet missing prod-web-01".to_owned()))?;
    let report = CapabilityReport::new(
        target.id.clone(),
        target.os,
        target
            .requested_capabilities
            .iter()
            .cloned()
            .map(Capability::new),
        [],
    );
    let plan = crate::plan_service_unhealthy_runbook(
        &crate::ServiceUnhealthyRunbookRequest::new(run_id, &target.id, "sshd"),
        &report,
    )
    .map_err(|error| E2eError::Planning(format!("{error:?}")))?;

    let identity = control_plane
        .agents()
        .iter()
        .find(|agent| agent.node_id == target.id)
        .ok_or_else(|| E2eError::Planning("target agent was not enrolled".to_owned()))?
        .identity();
    for (index, task) in plan
        .run
        .tasks
        .iter()
        .filter(|task| task.id.starts_with("collect-"))
        .enumerate()
    {
        let capability = task
            .required_capabilities
            .first()
            .cloned()
            .ok_or_else(|| E2eError::Planning(format!("{} missing capability", task.id)))?;
        let resource_id = task
            .reads
            .first()
            .cloned()
            .ok_or_else(|| E2eError::Planning(format!("{} missing read resource", task.id)))?;
        control_plane.enqueue_task(PendingAgentTask::new(
            AgentTaskEnvelope::new(
                format!("env-demo-{}", task.id),
                run_id,
                &task.id,
                &target.id,
                100,
                200,
                format!("nonce-demo-{}", task.id),
                task.required_capabilities.clone(),
                format!("audit-demo-{}", task.id),
            ),
            TypedTaskPayload::CollectEvidence {
                capability,
                resource_id,
            },
        ));
        let pulled = control_plane
            .pull_task(&identity, 101 + index as u64)
            .map_err(|error| E2eError::Planning(format!("{error:?}")))?;
        if !matches!(pulled.payload, TypedTaskPayload::CollectEvidence { .. }) {
            return Err(E2eError::Planning(format!(
                "{} pulled non-evidence payload",
                pulled.envelope.task_id
            )));
        }
        let evidence = evidence_for_collect_task(&pulled.envelope.task_id)?;
        control_plane
            .submit_result(
                &identity,
                AgentResultSubmission::new(
                    &pulled.envelope.envelope_id,
                    &pulled.envelope.run_id,
                    &pulled.envelope.task_id,
                    &pulled.envelope.node_id,
                    &pulled.envelope.nonce,
                    AgentResultStatus::Succeeded,
                    [evidence],
                    &pulled.envelope.audit_correlation_id,
                ),
                110 + index as u64,
            )
            .map_err(|error| E2eError::Planning(format!("{error:?}")))?;
    }
    let evidence: Vec<EvidenceEnvelope> = control_plane
        .accepted_results
        .iter()
        .filter(|result| result.run_id == run_id && result.status == AgentResultStatus::Succeeded)
        .flat_map(|result| result.evidence.clone())
        .collect();
    if evidence.len() != 5 {
        return Err(E2eError::Planning(format!(
            "expected 5 evidence results, got {}",
            evidence.len()
        )));
    }
    append_runtime_events(&mut ledger, run_id, control_plane.ledger.events())?;
    for evidence in &evidence {
        append(
            &mut ledger,
            run_id,
            AuditEventKind::EvidenceCollected {
                source: evidence.source.clone(),
            },
        )?;
    }

    let proposal = analyze_service_unhealthy(
        "proposal-demo-service-unhealthy",
        &target.id,
        "sshd",
        &evidence,
    );
    validate_proposal(&proposal, &ProposalPolicy::service_unhealthy())
        .map_err(|error| E2eError::Proposal(format!("{error:?}")))?;
    append(
        &mut ledger,
        run_id,
        AuditEventKind::ProposalGenerated {
            proposal_id: proposal.id.clone(),
            hypothesis: proposal.hypothesis.clone(),
        },
    )?;

    let restart_task = plan
        .run
        .tasks
        .iter()
        .find(|task| task.id == "restart-service")
        .ok_or_else(|| E2eError::Planning("restart-service task missing".to_owned()))?;
    let mut approvals = ApprovalStore::empty();
    approvals
        .request(ApprovalRecord::new(
            "approval-demo-service-unhealthy",
            run_id,
            &proposal,
            "restart-service",
            crate::OperationalLayer::System,
            "sshd",
            restart_task.impact.clone(),
            restart_task.verification.clone(),
            120,
            200,
        ))
        .map_err(|error| E2eError::Approval(format!("{error:?}")))?;
    let claims = approvals
        .approve(
            "approval-demo-service-unhealthy",
            "restart-service",
            "operator",
            150,
            "allow-prod-web-sshd-restart",
            "lease-nonce-demo",
        )
        .map_err(|error| E2eError::Approval(format!("{error:?}")))?;
    append_runtime_events(&mut ledger, run_id, approvals.ledger.events())?;
    append(
        &mut ledger,
        run_id,
        AuditEventKind::CapabilityLeaseIssued {
            lease_id: claims.lease_id.clone(),
            approval_id: claims.approval_id.clone(),
        },
    )?;
    append(
        &mut ledger,
        run_id,
        AuditEventKind::ResourceLeaseGranted {
            lease_id: claims.lease_id.clone(),
            resource_id: claims.target.resource_id.clone(),
            mode: LeaseMode::Exclusive,
        },
    )?;

    let signed = SignedCapabilityLease::new(claims.clone(), "demo-key", "demo-signature");
    let request = HelperActionRequest::new(
        &claims.lease_id,
        ActionKind::ServiceRestart,
        ActionTarget::new(&claims.target.resource_id, "sshd"),
        [HelperArgument::new("service", "sshd")],
    );
    let allowlist = HelperAllowlist::new([HelperAllowlistEntry::new(
        "allow-prod-web-sshd-restart",
        ActionKind::ServiceRestart,
        &claims.target.resource_id,
    )]);
    validate_helper_request(
        &request,
        &signed,
        &HelperValidationContext::new(
            "prod-web-01",
            150,
            LeaseSignatureStatus::Valid,
            [],
            allowlist,
        ),
    )
    .map_err(|error| E2eError::Helper(format!("{error:?}")))?;
    let helper_response = HelperActionResponse::new(
        HelperActionStatus::Succeeded,
        "dry-run service.restart validated",
    );
    append(
        &mut ledger,
        run_id,
        AuditEventKind::ActionResult {
            action: ActionKind::ServiceRestart,
            target: ActionTarget::new(&claims.target.resource_id, "sshd"),
            status: helper_response.status,
        },
    )?;

    let verification = VerificationPlanner::new(VerificationPolicy::minimal())
        .plan(&ActionKind::ServiceRestart, &restart_task.impact)
        .with_skipped(restart_task.verification.skipped.clone());
    append(
        &mut ledger,
        run_id,
        AuditEventKind::VerificationSelected {
            plan: verification.clone(),
        },
    )?;
    for check in &verification.required {
        append(
            &mut ledger,
            run_id,
            AuditEventKind::VerificationCompleted {
                check_id: check.id.clone(),
                resource_id: check.resource_id.clone(),
            },
        )?;
    }
    for skipped in &verification.skipped {
        append(
            &mut ledger,
            run_id,
            AuditEventKind::VerificationSkipped {
                skipped: skipped.clone(),
            },
        )?;
    }

    let receipt = generate_operator_receipt(run_id, &ledger)
        .map_err(|error| E2eError::Receipt(format!("{error:?}")))?;
    append(
        &mut ledger,
        run_id,
        AuditEventKind::CognitiveReceiptGenerated {
            receipt_id: "receipt-demo-service-unhealthy".to_owned(),
        },
    )?;

    Ok(ServiceUnhealthySimulation {
        run_id: run_id.to_owned(),
        stages: vec![
            JourneyStage::Declare,
            JourneyStage::Observe,
            JourneyStage::Propose,
            JourneyStage::Approve,
            JourneyStage::Lease,
            JourneyStage::Execute,
            JourneyStage::Verify,
            JourneyStage::Remember,
        ],
        receipt,
        ledger,
    })
}

fn append(ledger: &mut AuditLedger, run_id: &str, kind: AuditEventKind) -> Result<(), E2eError> {
    let sequence = ledger.next_sequence();
    ledger
        .append(AuditEvent::new(
            format!("e2e-event-{sequence}"),
            run_id,
            sequence,
            kind,
        ))
        .map_err(|error| E2eError::Receipt(format!("{error:?}")))
}

fn append_runtime_events(
    ledger: &mut AuditLedger,
    run_id: &str,
    events: &[AuditEvent],
) -> Result<(), E2eError> {
    for event in events {
        append(ledger, run_id, event.kind.clone())?;
    }
    Ok(())
}

fn evidence_for_collect_task(task_id: &str) -> Result<EvidenceEnvelope, E2eError> {
    let (source, body) = match task_id {
        "collect-service-status" => ("service_status", "service=not-active"),
        "collect-recent-logs" => ("recent_logs", "sshd failed with connection refused"),
        "collect-disk-snapshot" => ("disk_snapshot", "disk=present / 40%"),
        "collect-process-snapshot" => ("process_snapshot", "process=present sshd missing"),
        "collect-socket-snapshot" => ("socket_snapshot", "socket=present port 22 absent"),
        other => {
            return Err(E2eError::Planning(format!(
                "unsupported evidence task: {other}"
            )));
        }
    };
    Ok(runtime_text_evidence(source, body))
}

#[cfg(test)]
mod tests {
    use super::{JourneyStage, run_service_unhealthy_simulation};

    #[test]
    fn runs_full_service_unhealthy_simulation_from_example_fleet() {
        let simulation = run_service_unhealthy_simulation(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/fleet"
        ))
        .expect("e2e succeeds");
        assert_eq!(
            simulation.stages,
            [
                JourneyStage::Declare,
                JourneyStage::Observe,
                JourneyStage::Propose,
                JourneyStage::Approve,
                JourneyStage::Lease,
                JourneyStage::Execute,
                JourneyStage::Verify,
                JourneyStage::Remember,
            ]
        );
        let rendered = simulation.receipt.render_text();
        assert!(rendered.contains("incident-demo-service-unhealthy"));
        assert!(rendered.contains("service_status"));
        assert!(rendered.contains("package_audit"));
        assert!(rendered.contains("takeover"));
    }
}
