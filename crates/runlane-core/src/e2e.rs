use std::path::Path;

use crate::{
    ActionKind, ActionTarget, AgentIdentity, AgentResultStatus, AgentResultSubmission,
    AgentTaskEnvelope, AuditEvent, AuditEventKind, AuditLedger, Capability, CapabilityLeaseClaims,
    CapabilityReport, EvidenceEnvelope, HelperActionRequest, HelperActionResponse,
    HelperActionStatus, HelperAllowlist, HelperAllowlistEntry, HelperArgument,
    HelperValidationContext, ImpactSet, LeaseMode, LeaseSignatureStatus, SignedCapabilityLease,
    SkippedVerification, Task, VerificationPlanner, VerificationPolicy,
    analyzer::{
        ProposalPolicy, StructuredProposal, analyze_disk_pressure, analyze_service_unhealthy,
        validate_proposal,
    },
    approval::{ApprovalRecord, ApprovalStore},
    fleet::{FleetInventoryNode, FleetRepository},
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

/// Deterministic disk-pressure E2E simulation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskPressureSimulation {
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
    let fleet = load_fleet(fleet_path)?;
    let mut ledger = incident_ledger(run_id, incident_id, "prod-web-01", "service-unhealthy")?;
    let mut control_plane = ControlPlane::empty();
    enroll_fleet_agents(&mut control_plane, &fleet, EnrollmentLabels::service())?;

    let target = target_node(&fleet, "prod-web-01")?;
    let report = capability_report(target);
    let plan = crate::plan_service_unhealthy_runbook(
        &crate::ServiceUnhealthyRunbookRequest::new(run_id, &target.id, "sshd"),
        &report,
    )
    .map_err(|error| E2eError::Planning(format!("{error:?}")))?;

    let identity = enrolled_identity(&control_plane, &target.id)?;
    let evidence = collect_evidence(
        &mut control_plane,
        &identity,
        run_id,
        &target.id,
        &plan.run.tasks,
        evidence_for_collect_task,
    )?;
    append_observed_evidence(&mut ledger, run_id, &control_plane, &evidence)?;

    let proposal = service_unhealthy_proposal(&mut ledger, run_id, &target.id, &evidence)?;
    let restart_task = require_task(&plan.run.tasks, "restart-service")?;
    let claims = approve_action(
        &mut ledger,
        run_id,
        &proposal,
        restart_task,
        ApprovalFixture::service_unhealthy(),
    )?;
    validate_helper_action(
        &mut ledger,
        run_id,
        &claims,
        &ActionKind::ServiceRestart,
        HelperFixture::service_unhealthy(),
    )?;
    record_verification(
        &mut ledger,
        run_id,
        &ActionKind::ServiceRestart,
        &restart_task.impact,
        &restart_task.verification.skipped,
    )?;
    let receipt = finish_receipt(&mut ledger, run_id, "receipt-demo-service-unhealthy")?;

    Ok(ServiceUnhealthySimulation {
        run_id: run_id.to_owned(),
        stages: journey_stages(),
        receipt,
        ledger,
    })
}

/// Runs the deterministic disk-pressure journey against a fleet fixture.
pub fn run_disk_pressure_simulation(
    fleet_path: impl AsRef<Path>,
) -> Result<DiskPressureSimulation, E2eError> {
    let run_id = "run-demo-disk-pressure";
    let incident_id = "incident-demo-disk-pressure";
    let cleanup_resource_id = "system:node/prod-web-01/path/var-tmp-runlane-demo-cache";
    let cleanup_subject = "/var/tmp/runlane-demo-cache";
    let fleet = load_fleet(fleet_path)?;
    let mut ledger = incident_ledger(run_id, incident_id, "prod-web-01", "disk-pressure")?;
    let mut control_plane = ControlPlane::empty();
    enroll_fleet_agents(
        &mut control_plane,
        &fleet,
        EnrollmentLabels::disk_pressure(),
    )?;

    let target = target_node(&fleet, "prod-web-01")?;
    let report = capability_report(target);
    let plan = crate::plan_disk_pressure_runbook(
        &crate::DiskPressureRunbookRequest::new(
            run_id,
            &target.id,
            "/",
            cleanup_resource_id,
            cleanup_subject,
        ),
        &report,
    )
    .map_err(|error| E2eError::Planning(format!("{error:?}")))?;

    let identity = enrolled_identity(&control_plane, &target.id)?;
    let evidence = collect_evidence(
        &mut control_plane,
        &identity,
        run_id,
        &target.id,
        &plan.run.tasks,
        disk_pressure_evidence_for_collect_task,
    )?;
    append_observed_evidence(&mut ledger, run_id, &control_plane, &evidence)?;

    let proposal = disk_pressure_proposal(
        &mut ledger,
        run_id,
        &target.id,
        cleanup_resource_id,
        &evidence,
        &fleet,
    )?;
    let cleanup_task = require_task(&plan.run.tasks, "cleanup-allowlisted-path")?;
    let claims = approve_action(
        &mut ledger,
        run_id,
        &proposal,
        cleanup_task,
        ApprovalFixture::disk_pressure(cleanup_subject),
    )?;
    validate_helper_action(
        &mut ledger,
        run_id,
        &claims,
        &ActionKind::RemoveAllowlistedFile,
        HelperFixture::disk_pressure(cleanup_subject),
    )?;
    record_verification(
        &mut ledger,
        run_id,
        &ActionKind::RemoveAllowlistedFile,
        &cleanup_task.impact,
        &cleanup_task.verification.skipped,
    )?;
    let receipt = finish_receipt(&mut ledger, run_id, "receipt-demo-disk-pressure")?;

    Ok(DiskPressureSimulation {
        run_id: run_id.to_owned(),
        stages: journey_stages(),
        receipt,
        ledger,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnrollmentLabels {
    token: &'static str,
    enrollment: &'static str,
    certificate: &'static str,
    nonce: &'static str,
}

impl EnrollmentLabels {
    const fn service() -> Self {
        Self {
            token: "token",
            enrollment: "enroll",
            certificate: "cert",
            nonce: "enroll-nonce",
        }
    }

    const fn disk_pressure() -> Self {
        Self {
            token: "token-disk",
            enrollment: "enroll-disk",
            certificate: "cert-disk",
            nonce: "enroll-disk-nonce",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ApprovalFixture<'a> {
    approval_id: &'a str,
    action_id: &'a str,
    subject: &'a str,
    allowlist_entry_id: &'a str,
    lease_nonce: &'a str,
}

impl<'a> ApprovalFixture<'a> {
    const fn service_unhealthy() -> Self {
        Self {
            approval_id: "approval-demo-service-unhealthy",
            action_id: "restart-service",
            subject: "sshd",
            allowlist_entry_id: "allow-prod-web-sshd-restart",
            lease_nonce: "lease-nonce-demo",
        }
    }

    const fn disk_pressure(cleanup_subject: &'a str) -> Self {
        Self {
            approval_id: "approval-demo-disk-pressure",
            action_id: "remove-allowlisted-cleanup-path",
            subject: cleanup_subject,
            allowlist_entry_id: "allow-prod-web-runlane-demo-cache-cleanup",
            lease_nonce: "lease-nonce-demo-disk-pressure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HelperFixture<'a> {
    node_id: &'a str,
    subject: &'a str,
    argument_name: &'a str,
    argument_value: &'a str,
    allowlist_entry_id: &'a str,
    success_message: &'a str,
}

impl<'a> HelperFixture<'a> {
    const fn service_unhealthy() -> Self {
        Self {
            node_id: "prod-web-01",
            subject: "sshd",
            argument_name: "service",
            argument_value: "sshd",
            allowlist_entry_id: "allow-prod-web-sshd-restart",
            success_message: "dry-run service.restart validated",
        }
    }

    const fn disk_pressure(cleanup_subject: &'a str) -> Self {
        Self {
            node_id: "prod-web-01",
            subject: cleanup_subject,
            argument_name: "path",
            argument_value: cleanup_subject,
            allowlist_entry_id: "allow-prod-web-runlane-demo-cache-cleanup",
            success_message: "dry-run file.remove_from_allowlist validated",
        }
    }
}

fn journey_stages() -> Vec<JourneyStage> {
    vec![
        JourneyStage::Declare,
        JourneyStage::Observe,
        JourneyStage::Propose,
        JourneyStage::Approve,
        JourneyStage::Lease,
        JourneyStage::Execute,
        JourneyStage::Verify,
        JourneyStage::Remember,
    ]
}

fn load_fleet(fleet_path: impl AsRef<Path>) -> Result<FleetRepository, E2eError> {
    FleetRepository::load(fleet_path).map_err(|error| E2eError::Fleet(error.to_string()))
}

fn incident_ledger(
    run_id: &str,
    incident_id: &str,
    node_id: &str,
    runbook: &str,
) -> Result<AuditLedger, E2eError> {
    let mut ledger = AuditLedger::empty();
    append(
        &mut ledger,
        run_id,
        AuditEventKind::IncidentCreated {
            incident_id: incident_id.to_owned(),
            node_id: node_id.to_owned(),
            runbook: runbook.to_owned(),
        },
    )?;
    Ok(ledger)
}

fn enroll_fleet_agents(
    control_plane: &mut ControlPlane,
    fleet: &FleetRepository,
    labels: EnrollmentLabels,
) -> Result<(), E2eError> {
    for node in &fleet.inventory {
        control_plane
            .create_enrollment_token(EnrollmentToken::new(
                format!("{}-{}", labels.token, node.id),
                format!("{}-{}", labels.enrollment, node.id),
                node.id.clone(),
                node.os,
                "demo-trust-root",
                200,
                format!("{}-{}", labels.nonce, node.id),
            ))
            .map_err(|error| E2eError::Planning(format!("{error:?}")))?;
        control_plane
            .enroll_agent(&AgentEnrollmentRequest::new(
                format!("{}-{}", labels.enrollment, node.id),
                node.id.clone(),
                node.os,
                format!("{}-{}", labels.certificate, node.id),
                "demo-trust-root",
                100,
            ))
            .map_err(|error| E2eError::Planning(format!("{error:?}")))?;
    }
    Ok(())
}

fn target_node<'a>(
    fleet: &'a FleetRepository,
    node_id: &str,
) -> Result<&'a FleetInventoryNode, E2eError> {
    fleet
        .inventory
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| E2eError::Fleet(format!("examples/fleet missing {node_id}")))
}

fn capability_report(target: &FleetInventoryNode) -> CapabilityReport {
    CapabilityReport::new(
        target.id.clone(),
        target.os,
        target
            .requested_capabilities
            .iter()
            .cloned()
            .map(Capability::new),
        [],
    )
}

fn enrolled_identity(
    control_plane: &ControlPlane,
    node_id: &str,
) -> Result<AgentIdentity, E2eError> {
    control_plane
        .agents()
        .iter()
        .find(|agent| agent.node_id == node_id)
        .ok_or_else(|| E2eError::Planning("target agent was not enrolled".to_owned()))
        .map(crate::runtime::AgentIdentityRecord::identity)
}

fn collect_evidence(
    control_plane: &mut ControlPlane,
    identity: &AgentIdentity,
    run_id: &str,
    target_id: &str,
    tasks: &[Task],
    evidence_for_task: fn(&str) -> Result<EvidenceEnvelope, E2eError>,
) -> Result<Vec<EvidenceEnvelope>, E2eError> {
    let expected_count = tasks
        .iter()
        .filter(|task| task.id.starts_with("collect-"))
        .count();
    for (index, task) in tasks
        .iter()
        .filter(|task| task.id.starts_with("collect-"))
        .enumerate()
    {
        enqueue_collect_task(control_plane, run_id, target_id, task)?;
        let pulled = control_plane
            .pull_task(identity, 101 + index as u64)
            .map_err(|error| E2eError::Planning(format!("{error:?}")))?;
        if !matches!(pulled.payload, TypedTaskPayload::CollectEvidence { .. }) {
            return Err(E2eError::Planning(format!(
                "{} pulled non-evidence payload",
                pulled.envelope.task_id
            )));
        }
        let evidence = evidence_for_task(&pulled.envelope.task_id)?;
        control_plane
            .submit_result(
                identity,
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
    let evidence = accepted_evidence(control_plane, run_id);
    if evidence.len() != expected_count {
        return Err(E2eError::Planning(format!(
            "expected {} evidence results, got {}",
            expected_count,
            evidence.len()
        )));
    }
    Ok(evidence)
}

fn enqueue_collect_task(
    control_plane: &mut ControlPlane,
    run_id: &str,
    target_id: &str,
    task: &Task,
) -> Result<(), E2eError> {
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
            target_id,
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
    Ok(())
}

fn accepted_evidence(control_plane: &ControlPlane, run_id: &str) -> Vec<EvidenceEnvelope> {
    control_plane
        .accepted_results
        .iter()
        .filter(|result| result.run_id == run_id && result.status == AgentResultStatus::Succeeded)
        .flat_map(|result| result.evidence.clone())
        .collect()
}

fn append_observed_evidence(
    ledger: &mut AuditLedger,
    run_id: &str,
    control_plane: &ControlPlane,
    evidence: &[EvidenceEnvelope],
) -> Result<(), E2eError> {
    append_runtime_events(ledger, run_id, control_plane.ledger.events())?;
    for item in evidence {
        append(
            ledger,
            run_id,
            AuditEventKind::EvidenceCollected {
                source: item.source.clone(),
            },
        )?;
    }
    Ok(())
}

fn service_unhealthy_proposal(
    ledger: &mut AuditLedger,
    run_id: &str,
    target_id: &str,
    evidence: &[EvidenceEnvelope],
) -> Result<StructuredProposal, E2eError> {
    let proposal = analyze_service_unhealthy(
        "proposal-demo-service-unhealthy",
        target_id,
        "sshd",
        evidence,
    );
    validate_and_record_proposal(
        ledger,
        run_id,
        proposal,
        &ProposalPolicy::service_unhealthy(),
    )
}

fn disk_pressure_proposal(
    ledger: &mut AuditLedger,
    run_id: &str,
    target_id: &str,
    cleanup_resource_id: &str,
    evidence: &[EvidenceEnvelope],
    fleet: &FleetRepository,
) -> Result<StructuredProposal, E2eError> {
    let allowed_cleanup_resources = fleet
        .allowlists
        .iter()
        .filter(|entry| entry.action == "file.remove_from_allowlist")
        .map(|entry| entry.target_resource_id.clone())
        .collect::<Vec<_>>();
    let proposal = analyze_disk_pressure(
        "proposal-demo-disk-pressure",
        target_id,
        "/",
        cleanup_resource_id,
        evidence,
        &allowed_cleanup_resources,
    );
    validate_and_record_proposal(ledger, run_id, proposal, &ProposalPolicy::disk_pressure())
}

fn validate_and_record_proposal(
    ledger: &mut AuditLedger,
    run_id: &str,
    proposal: StructuredProposal,
    policy: &ProposalPolicy,
) -> Result<StructuredProposal, E2eError> {
    validate_proposal(&proposal, policy)
        .map_err(|error| E2eError::Proposal(format!("{error:?}")))?;
    append(
        ledger,
        run_id,
        AuditEventKind::ProposalGenerated {
            proposal_id: proposal.id.clone(),
            hypothesis: proposal.hypothesis.clone(),
        },
    )?;
    Ok(proposal)
}

fn require_task<'a>(tasks: &'a [Task], task_id: &str) -> Result<&'a Task, E2eError> {
    tasks
        .iter()
        .find(|task| task.id == task_id)
        .ok_or_else(|| E2eError::Planning(format!("{task_id} task missing")))
}

fn approve_action(
    ledger: &mut AuditLedger,
    run_id: &str,
    proposal: &StructuredProposal,
    task: &Task,
    fixture: ApprovalFixture<'_>,
) -> Result<CapabilityLeaseClaims, E2eError> {
    let mut approvals = ApprovalStore::empty();
    approvals
        .request(ApprovalRecord::new(
            fixture.approval_id,
            run_id,
            proposal,
            fixture.action_id,
            crate::OperationalLayer::System,
            fixture.subject,
            task.impact.clone(),
            task.verification.clone(),
            120,
            200,
        ))
        .map_err(|error| E2eError::Approval(format!("{error:?}")))?;
    let claims = approvals
        .approve(
            fixture.approval_id,
            fixture.action_id,
            "operator",
            150,
            fixture.allowlist_entry_id,
            fixture.lease_nonce,
        )
        .map_err(|error| E2eError::Approval(format!("{error:?}")))?;
    append_runtime_events(ledger, run_id, approvals.ledger.events())?;
    append(
        ledger,
        run_id,
        AuditEventKind::CapabilityLeaseIssued {
            lease_id: claims.lease_id.clone(),
            approval_id: claims.approval_id.clone(),
        },
    )?;
    append(
        ledger,
        run_id,
        AuditEventKind::ResourceLeaseGranted {
            lease_id: claims.lease_id.clone(),
            resource_id: claims.target.resource_id.clone(),
            mode: LeaseMode::Exclusive,
        },
    )?;
    Ok(claims)
}

fn validate_helper_action(
    ledger: &mut AuditLedger,
    run_id: &str,
    claims: &CapabilityLeaseClaims,
    action: &ActionKind,
    fixture: HelperFixture<'_>,
) -> Result<(), E2eError> {
    let signed = SignedCapabilityLease::new(claims.clone(), "demo-key", "demo-signature");
    let request = HelperActionRequest::new(
        &claims.lease_id,
        action.clone(),
        ActionTarget::new(&claims.target.resource_id, fixture.subject),
        [HelperArgument::new(
            fixture.argument_name,
            fixture.argument_value,
        )],
    );
    let allowlist = HelperAllowlist::new([HelperAllowlistEntry::new(
        fixture.allowlist_entry_id,
        action.clone(),
        &claims.target.resource_id,
    )]);
    validate_helper_request(
        &request,
        &signed,
        &HelperValidationContext::new(
            fixture.node_id,
            150,
            LeaseSignatureStatus::Valid,
            [],
            allowlist,
        ),
    )
    .map_err(|error| E2eError::Helper(format!("{error:?}")))?;
    let helper_response =
        HelperActionResponse::new(HelperActionStatus::Succeeded, fixture.success_message);
    append(
        ledger,
        run_id,
        AuditEventKind::ActionResult {
            action: action.clone(),
            target: ActionTarget::new(&claims.target.resource_id, fixture.subject),
            status: helper_response.status,
        },
    )
}

fn record_verification(
    ledger: &mut AuditLedger,
    run_id: &str,
    action: &ActionKind,
    impact: &ImpactSet,
    skipped: &[SkippedVerification],
) -> Result<(), E2eError> {
    let verification = VerificationPlanner::new(VerificationPolicy::minimal())
        .plan(action, impact)
        .with_skipped(skipped.iter().cloned());
    append(
        ledger,
        run_id,
        AuditEventKind::VerificationSelected {
            plan: verification.clone(),
        },
    )?;
    for check in &verification.required {
        append(
            ledger,
            run_id,
            AuditEventKind::VerificationCompleted {
                check_id: check.id.clone(),
                resource_id: check.resource_id.clone(),
            },
        )?;
    }
    for skipped in &verification.skipped {
        append(
            ledger,
            run_id,
            AuditEventKind::VerificationSkipped {
                skipped: skipped.clone(),
            },
        )?;
    }
    Ok(())
}

fn finish_receipt(
    ledger: &mut AuditLedger,
    run_id: &str,
    receipt_id: &str,
) -> Result<OperatorReceipt, E2eError> {
    let receipt = generate_operator_receipt(run_id, ledger)
        .map_err(|error| E2eError::Receipt(format!("{error:?}")))?;
    append(
        ledger,
        run_id,
        AuditEventKind::CognitiveReceiptGenerated {
            receipt_id: receipt_id.to_owned(),
        },
    )?;
    Ok(receipt)
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

fn disk_pressure_evidence_for_collect_task(task_id: &str) -> Result<EvidenceEnvelope, E2eError> {
    let (source, body) = match task_id {
        "collect-disk-usage" => ("disk_usage", "disk_pressure=high mount=/ usage=92"),
        "collect-cleanup-candidate" => (
            "cleanup_candidate",
            "cleanup_candidate=present path=/var/tmp/runlane-demo-cache bytes=524288000",
        ),
        "collect-disk-pressure-logs" => (
            "disk_pressure_logs",
            "log=present disk pressure detected; no deletion command supplied",
        ),
        "collect-pre-action-snapshot" => (
            "pre_action_snapshot",
            "pre_action_snapshot=present free_space=low cleanup_candidate=present",
        ),
        other => {
            return Err(E2eError::Planning(format!(
                "unsupported disk-pressure evidence task: {other}"
            )));
        }
    };
    Ok(runtime_text_evidence(source, body))
}

#[cfg(test)]
mod tests {
    use super::{JourneyStage, run_disk_pressure_simulation, run_service_unhealthy_simulation};

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

    #[test]
    fn runs_full_disk_pressure_simulation_from_example_fleet() {
        let simulation = run_disk_pressure_simulation(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/fleet"
        ))
        .expect("disk-pressure e2e succeeds");
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
        assert!(rendered.contains("incident-demo-disk-pressure"));
        assert!(rendered.contains("cleanup_candidate"));
        assert!(rendered.contains("free_space_improved"));
        assert!(rendered.contains("cleaned_paths_match_allowlist"));
        assert!(rendered.contains("no_unrelated_deletion_reported"));
    }
}
