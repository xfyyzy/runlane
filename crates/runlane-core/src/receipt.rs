use crate::{
    ActionKind, AuditEventKind, AuditLedger, HelperActionStatus, OperationalLayer,
    SkippedVerification, VerificationCheck,
};

/// Operator-facing incident receipt reconstructed from audit events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorReceipt {
    pub run_id: String,
    pub incident_id: String,
    pub node_id: String,
    pub layer: OperationalLayer,
    pub changed_resources: Vec<String>,
    pub evidence_references: Vec<String>,
    pub proposal_id: String,
    pub hypothesis: String,
    pub approval_id: String,
    pub lease_id: String,
    pub helper_action: ActionKind,
    pub helper_status: HelperActionStatus,
    pub verification_checks: Vec<VerificationCheck>,
    pub verification_completed: Vec<String>,
    pub skipped_checks: Vec<SkippedVerification>,
    pub residual_risk: String,
    pub takeover_notes: String,
}

impl OperatorReceipt {
    /// Renders a deterministic text receipt for CLI output and snapshot tests.
    #[must_use]
    pub fn render_text(&self) -> String {
        format!(
            "run: {}\nincident: {}\nnode: {}\nlayer: {:?}\nchanged: {}\nevidence: {}\nproposal: {} - {}\napproval: {}\nlease: {}\nhelper: {:?} {:?}\nverified: {}\nskipped: {}\nresidual_risk: {}\ntakeover: {}",
            self.run_id,
            self.incident_id,
            self.node_id,
            self.layer,
            self.changed_resources.join(","),
            self.evidence_references.join(","),
            self.proposal_id,
            self.hypothesis,
            self.approval_id,
            self.lease_id,
            self.helper_action,
            self.helper_status,
            self.verification_completed.join(","),
            self.skipped_checks
                .iter()
                .map(|skipped| format!("{} ({})", skipped.check_id, skipped.reason))
                .collect::<Vec<_>>()
                .join(","),
            self.residual_risk,
            self.takeover_notes
        )
    }
}

/// Receipt generation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptError {
    MissingEvent(&'static str),
}

/// Generates a receipt from audit ledger events.
pub fn generate_operator_receipt(
    run_id: &str,
    ledger: &AuditLedger,
) -> Result<OperatorReceipt, ReceiptError> {
    let mut incident_id = None;
    let mut node_id = None;
    let mut evidence_references = Vec::new();
    let mut proposal_id = None;
    let mut hypothesis = None;
    let mut approval_id = None;
    let mut lease_id = None;
    let mut changed_resources = Vec::new();
    let mut helper_action = None;
    let mut helper_status = None;
    let mut verification_checks = Vec::new();
    let mut verification_completed = Vec::new();
    let mut skipped_checks = Vec::new();

    for event in ledger
        .events()
        .iter()
        .filter(|event| event.run_id == run_id)
    {
        match &event.kind {
            AuditEventKind::IncidentCreated {
                incident_id: id,
                node_id: node,
                ..
            } => {
                incident_id = Some(id.clone());
                node_id = Some(node.clone());
            }
            AuditEventKind::EvidenceCollected { source } => {
                evidence_references.push(source.clone());
            }
            AuditEventKind::ProposalGenerated {
                proposal_id: id,
                hypothesis: text,
            } => {
                proposal_id = Some(id.clone());
                hypothesis = Some(text.clone());
            }
            AuditEventKind::ApprovalDecision {
                approval_id: id,
                outcome: crate::ApprovalOutcome::Approved,
                ..
            } => {
                approval_id = Some(id.clone());
            }
            AuditEventKind::CapabilityLeaseIssued {
                lease_id: id,
                approval_id: approved,
            } => {
                lease_id = Some(id.clone());
                approval_id.get_or_insert_with(|| approved.clone());
            }
            AuditEventKind::ResourceLeaseGranted { resource_id, .. } => {
                changed_resources.push(resource_id.clone());
            }
            AuditEventKind::ActionResult {
                action,
                status,
                target,
            } => {
                helper_action = Some(action.clone());
                helper_status = Some(*status);
                changed_resources.push(target.resource_id.clone());
            }
            AuditEventKind::VerificationSelected { plan } => {
                verification_checks.extend(plan.required.clone());
                skipped_checks.extend(plan.skipped.clone());
            }
            AuditEventKind::VerificationCompleted {
                check_id,
                resource_id,
            } => {
                verification_completed.push(format!("{check_id}:{resource_id}"));
            }
            AuditEventKind::VerificationSkipped { skipped } => {
                skipped_checks.push(skipped.clone());
            }
            _ => {}
        }
    }

    let incident_id = incident_id.ok_or(ReceiptError::MissingEvent("incident_created"))?;
    let node_id = node_id.ok_or(ReceiptError::MissingEvent("incident_node"))?;
    if evidence_references.is_empty() {
        return Err(ReceiptError::MissingEvent("evidence_collected"));
    }
    let proposal_id = proposal_id.ok_or(ReceiptError::MissingEvent("proposal_generated"))?;
    let hypothesis = hypothesis.ok_or(ReceiptError::MissingEvent("proposal_hypothesis"))?;
    let approval_id = approval_id.ok_or(ReceiptError::MissingEvent("approval_approved"))?;
    let lease_id = lease_id.ok_or(ReceiptError::MissingEvent("capability_lease_issued"))?;
    let helper_action = helper_action.ok_or(ReceiptError::MissingEvent("helper_action_result"))?;
    let helper_status = helper_status.ok_or(ReceiptError::MissingEvent("helper_action_status"))?;
    if verification_checks.is_empty() {
        return Err(ReceiptError::MissingEvent("verification_selected"));
    }
    if verification_completed.is_empty() {
        return Err(ReceiptError::MissingEvent("verification_completed"));
    }

    changed_resources.sort();
    changed_resources.dedup();
    skipped_checks.sort_by(|left, right| left.check_id.cmp(&right.check_id));
    skipped_checks.dedup_by(|left, right| left.check_id == right.check_id);

    let residual_risk = match helper_action {
        ActionKind::RemoveAllowlistedFile => {
            "disk pressure may recur until the growth source is fixed; cleanup was limited to the allowlist"
        }
        _ => "root cause still requires operator review if service fails again",
    };
    let takeover_notes = match helper_action {
        ActionKind::RemoveAllowlistedFile => {
            "operator can inspect disk evidence, cleanup allowlist, and verification before broader cleanup"
        }
        _ => "operator can inspect collected evidence and rerun verification",
    };

    Ok(OperatorReceipt {
        run_id: run_id.to_owned(),
        incident_id,
        node_id,
        layer: OperationalLayer::System,
        changed_resources,
        evidence_references,
        proposal_id,
        hypothesis,
        approval_id,
        lease_id,
        helper_action,
        helper_status,
        verification_checks,
        verification_completed,
        skipped_checks,
        residual_risk: residual_risk.to_owned(),
        takeover_notes: takeover_notes.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        ActionKind, ActionTarget, ApprovalOutcome, AuditEvent, AuditEventKind, AuditLedger,
        HelperActionStatus, LeaseMode, OperationalLayer, SkippedVerification, VerificationCheck,
        VerificationPlan, VerificationTier,
    };

    use super::{ReceiptError, generate_operator_receipt};

    #[test]
    fn generates_receipt_from_complete_ledger() {
        let ledger = complete_ledger();
        let receipt =
            generate_operator_receipt("run-1", &ledger).expect("complete ledger renders receipt");
        assert_eq!(receipt.incident_id, "incident-1");
        assert_eq!(receipt.node_id, "prod-web-01");
        assert_eq!(receipt.layer, OperationalLayer::System);
        assert_eq!(receipt.approval_id, "approval-1");
        assert_eq!(receipt.lease_id, "lease-1");
        assert!(receipt.render_text().contains("package_audit"));
        assert!(receipt.render_text().contains("takeover"));
    }

    #[test]
    fn incomplete_ledger_fails_explicitly() {
        let mut ledger = AuditLedger::empty();
        append(
            &mut ledger,
            AuditEventKind::IncidentCreated {
                incident_id: "incident-1".to_owned(),
                node_id: "prod-web-01".to_owned(),
                runbook: "service-unhealthy".to_owned(),
            },
        );

        assert_eq!(
            generate_operator_receipt("run-1", &ledger),
            Err(ReceiptError::MissingEvent("evidence_collected"))
        );
    }

    fn complete_ledger() -> AuditLedger {
        let mut ledger = AuditLedger::empty();
        append(
            &mut ledger,
            AuditEventKind::IncidentCreated {
                incident_id: "incident-1".to_owned(),
                node_id: "prod-web-01".to_owned(),
                runbook: "service-unhealthy".to_owned(),
            },
        );
        append(
            &mut ledger,
            AuditEventKind::EvidenceCollected {
                source: "service_status".to_owned(),
            },
        );
        append(
            &mut ledger,
            AuditEventKind::ProposalGenerated {
                proposal_id: "proposal-1".to_owned(),
                hypothesis: "sshd appears unhealthy".to_owned(),
            },
        );
        append(
            &mut ledger,
            AuditEventKind::ApprovalDecision {
                approval_id: "approval-1".to_owned(),
                actor: "operator".to_owned(),
                outcome: ApprovalOutcome::Approved,
            },
        );
        append(
            &mut ledger,
            AuditEventKind::CapabilityLeaseIssued {
                lease_id: "lease-1".to_owned(),
                approval_id: "approval-1".to_owned(),
            },
        );
        append(
            &mut ledger,
            AuditEventKind::ResourceLeaseGranted {
                lease_id: "lease-1".to_owned(),
                resource_id: "system:node/prod-web-01/service/sshd".to_owned(),
                mode: LeaseMode::Exclusive,
            },
        );
        let verification = VerificationPlan::required([VerificationCheck::new(
            "service_active",
            "system:node/prod-web-01/service/sshd",
            VerificationTier::DirectImpact,
        )])
        .with_skipped([SkippedVerification::new(
            "package_audit",
            "service restart did not mutate package database",
        )]);
        append(
            &mut ledger,
            AuditEventKind::VerificationSelected {
                plan: verification.clone(),
            },
        );
        append(
            &mut ledger,
            AuditEventKind::ActionResult {
                action: ActionKind::ServiceRestart,
                target: ActionTarget::new("system:node/prod-web-01/service/sshd", "sshd"),
                status: HelperActionStatus::Succeeded,
            },
        );
        append(
            &mut ledger,
            AuditEventKind::VerificationCompleted {
                check_id: "service_active".to_owned(),
                resource_id: "system:node/prod-web-01/service/sshd".to_owned(),
            },
        );
        append(
            &mut ledger,
            AuditEventKind::VerificationSkipped {
                skipped: verification.skipped[0].clone(),
            },
        );
        ledger
    }

    fn append(ledger: &mut AuditLedger, kind: AuditEventKind) {
        let sequence = ledger.next_sequence();
        ledger
            .append(AuditEvent::new("event", "run-1", sequence, kind))
            .expect("event sequence is valid");
    }
}
