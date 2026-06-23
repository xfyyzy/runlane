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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ReceiptParts {
    incident_id: Option<String>,
    node_id: Option<String>,
    evidence_references: Vec<String>,
    proposal_id: Option<String>,
    hypothesis: Option<String>,
    approval_id: Option<String>,
    lease_id: Option<String>,
    changed_resources: Vec<String>,
    helper_action: Option<ActionKind>,
    helper_status: Option<HelperActionStatus>,
    verification_checks: Vec<VerificationCheck>,
    verification_completed: Vec<String>,
    skipped_checks: Vec<SkippedVerification>,
}

/// Generates a receipt from audit ledger events.
pub fn generate_operator_receipt(
    run_id: &str,
    ledger: &AuditLedger,
) -> Result<OperatorReceipt, ReceiptError> {
    collect_receipt_parts(run_id, ledger).finalize(run_id)
}

fn collect_receipt_parts(run_id: &str, ledger: &AuditLedger) -> ReceiptParts {
    let mut parts = ReceiptParts::default();
    for event in ledger
        .events()
        .iter()
        .filter(|event| event.run_id == run_id)
    {
        parts.apply(&event.kind);
    }
    parts
}

impl ReceiptParts {
    fn apply(&mut self, kind: &AuditEventKind) {
        match kind {
            AuditEventKind::IncidentCreated {
                incident_id: id,
                node_id: node,
                ..
            } => {
                self.incident_id = Some(id.clone());
                self.node_id = Some(node.clone());
            }
            AuditEventKind::EvidenceCollected { source } => {
                self.evidence_references.push(source.clone());
            }
            AuditEventKind::ProposalGenerated {
                proposal_id: id,
                hypothesis: text,
            } => {
                self.proposal_id = Some(id.clone());
                self.hypothesis = Some(text.clone());
            }
            AuditEventKind::ApprovalDecision {
                approval_id: id,
                outcome: crate::ApprovalOutcome::Approved,
                ..
            } => {
                self.approval_id = Some(id.clone());
            }
            AuditEventKind::CapabilityLeaseIssued {
                lease_id: id,
                approval_id: approved,
            } => {
                self.lease_id = Some(id.clone());
                self.approval_id.get_or_insert_with(|| approved.clone());
            }
            AuditEventKind::ResourceLeaseGranted { resource_id, .. } => {
                self.changed_resources.push(resource_id.clone());
            }
            AuditEventKind::ActionResult {
                action,
                status,
                target,
            } => {
                self.helper_action = Some(action.clone());
                self.helper_status = Some(*status);
                self.changed_resources.push(target.resource_id.clone());
            }
            AuditEventKind::VerificationSelected { plan } => {
                self.verification_checks.extend(plan.required.clone());
                self.skipped_checks.extend(plan.skipped.clone());
            }
            AuditEventKind::VerificationCompleted {
                check_id,
                resource_id,
            } => {
                self.verification_completed
                    .push(format!("{check_id}:{resource_id}"));
            }
            AuditEventKind::VerificationSkipped { skipped } => {
                self.skipped_checks.push(skipped.clone());
            }
            _ => {}
        }
    }

    fn finalize(mut self, run_id: &str) -> Result<OperatorReceipt, ReceiptError> {
        let incident_id = self
            .incident_id
            .ok_or(ReceiptError::MissingEvent("incident_created"))?;
        let node_id = self
            .node_id
            .ok_or(ReceiptError::MissingEvent("incident_node"))?;
        if self.evidence_references.is_empty() {
            return Err(ReceiptError::MissingEvent("evidence_collected"));
        }
        let proposal_id = self
            .proposal_id
            .ok_or(ReceiptError::MissingEvent("proposal_generated"))?;
        let hypothesis = self
            .hypothesis
            .ok_or(ReceiptError::MissingEvent("proposal_hypothesis"))?;
        let approval_id = self
            .approval_id
            .ok_or(ReceiptError::MissingEvent("approval_approved"))?;
        let lease_id = self
            .lease_id
            .ok_or(ReceiptError::MissingEvent("capability_lease_issued"))?;
        let helper_action = self
            .helper_action
            .ok_or(ReceiptError::MissingEvent("helper_action_result"))?;
        let helper_status = self
            .helper_status
            .ok_or(ReceiptError::MissingEvent("helper_action_status"))?;
        if self.verification_checks.is_empty() {
            return Err(ReceiptError::MissingEvent("verification_selected"));
        }
        if self.verification_completed.is_empty() {
            return Err(ReceiptError::MissingEvent("verification_completed"));
        }

        self.changed_resources.sort();
        self.changed_resources.dedup();
        self.skipped_checks
            .sort_by(|left, right| left.check_id.cmp(&right.check_id));
        self.skipped_checks
            .dedup_by(|left, right| left.check_id == right.check_id);

        let (residual_risk, takeover_notes) = receipt_operator_notes(&helper_action);

        Ok(OperatorReceipt {
            run_id: run_id.to_owned(),
            incident_id,
            node_id,
            layer: OperationalLayer::System,
            changed_resources: self.changed_resources,
            evidence_references: self.evidence_references,
            proposal_id,
            hypothesis,
            approval_id,
            lease_id,
            helper_action,
            helper_status,
            verification_checks: self.verification_checks,
            verification_completed: self.verification_completed,
            skipped_checks: self.skipped_checks,
            residual_risk: residual_risk.to_owned(),
            takeover_notes: takeover_notes.to_owned(),
        })
    }
}

fn receipt_operator_notes(helper_action: &ActionKind) -> (&'static str, &'static str) {
    match helper_action {
        ActionKind::RemoveAllowlistedFile => (
            "disk pressure may recur until the growth source is fixed; cleanup was limited to the allowlist",
            "operator can inspect disk evidence, cleanup allowlist, and verification before broader cleanup",
        ),
        _ => (
            "root cause still requires operator review if service fails again",
            "operator can inspect collected evidence and rerun verification",
        ),
    }
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
