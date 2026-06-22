use crate::{
    ActionTarget, ApprovalOutcome, AuditAppendError, AuditEvent, AuditEventKind, AuditLedger,
    CapabilityLeaseClaims, ImpactSet, LeaseMode, OperationalLayer, ResourceLeaseRequest,
    SkippedVerification, VerificationPlan,
    analyzer::{ProposedActionKind, StructuredProposal},
};

/// Approval lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Expired,
    Superseded,
}

/// Runtime approval record bound to exactly one proposal action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRecord {
    pub id: String,
    pub run_id: String,
    pub proposal_id: String,
    pub action_id: String,
    pub action: ProposedActionKind,
    pub layer: OperationalLayer,
    pub target: ActionTarget,
    pub impact: ImpactSet,
    pub lease_request: ResourceLeaseRequest,
    pub verification: VerificationPlan,
    pub evidence_references: Vec<String>,
    pub state: ApprovalState,
    pub requested_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

impl ApprovalRecord {
    /// Creates an approval request for one proposed action.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        run_id: impl Into<String>,
        proposal: &StructuredProposal,
        action_id: impl Into<String>,
        layer: OperationalLayer,
        subject: impl Into<String>,
        impact: ImpactSet,
        verification: VerificationPlan,
        requested_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    ) -> Self {
        let action_id = action_id.into();
        let action = proposal
            .proposed_actions
            .iter()
            .find(|action| action.id == action_id)
            .expect("approval action id must exist in proposal");
        let lease_request = action.lease_request.clone().unwrap_or_else(|| {
            ResourceLeaseRequest::new(
                action.target_resource_id.clone(),
                LeaseMode::Exclusive,
                "approval-bound default exclusive lease",
            )
        });
        Self {
            id: id.into(),
            run_id: run_id.into(),
            proposal_id: proposal.id.clone(),
            action_id: action.id.clone(),
            action: action.kind,
            layer,
            target: ActionTarget::new(action.target_resource_id.clone(), subject),
            impact,
            lease_request,
            verification,
            evidence_references: proposal.evidence_references.clone(),
            state: ApprovalState::Pending,
            requested_at_unix_seconds,
            expires_at_unix_seconds,
        }
    }
}

/// Approval operation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    UnknownApproval,
    NotPending,
    StaleApproval,
    ActionMismatch,
    UnsupportedHelperAction,
    AuditAppend(AuditAppendError),
}

/// In-memory approval API boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalStore {
    records: Vec<ApprovalRecord>,
    pub ledger: AuditLedger,
}

impl ApprovalStore {
    /// Creates an empty approval store.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            records: Vec::new(),
            ledger: AuditLedger::empty(),
        }
    }

    /// Requests approval for a bound proposal action.
    pub fn request(&mut self, record: ApprovalRecord) -> Result<(), ApprovalError> {
        self.append_event(AuditEventKind::ApprovalRequested {
            approval_id: record.id.clone(),
            proposal_id: record.proposal_id.clone(),
            action_id: record.action_id.clone(),
        })?;
        self.records.push(record);
        Ok(())
    }

    /// Lists pending approvals.
    #[must_use]
    pub fn list_pending(&self) -> Vec<&ApprovalRecord> {
        self.records
            .iter()
            .filter(|record| record.state == ApprovalState::Pending)
            .collect()
    }

    /// Shows an approval by id.
    #[must_use]
    pub fn show(&self, id: &str) -> Option<&ApprovalRecord> {
        self.records.iter().find(|record| record.id == id)
    }

    /// Approves an exact proposal action and returns lease claims to sign.
    pub fn approve(
        &mut self,
        id: &str,
        expected_action_id: &str,
        actor: &str,
        now_unix_seconds: u64,
        allowlist_entry_id: &str,
        lease_nonce: &str,
    ) -> Result<CapabilityLeaseClaims, ApprovalError> {
        let index = self.index_of(id)?;
        let record = self.records[index].clone();
        self.ensure_pending_not_stale(&record, now_unix_seconds)?;
        if record.action_id != expected_action_id {
            return Err(ApprovalError::ActionMismatch);
        }
        let helper_action = record
            .action
            .helper_action()
            .ok_or(ApprovalError::UnsupportedHelperAction)?;
        let claims = CapabilityLeaseClaims::new(
            format!("lease-{id}"),
            record.run_id.clone(),
            record.id.clone(),
            node_from_target(&record.target.resource_id),
            helper_action,
            record.target.clone(),
            allowlist_entry_id,
            record.expires_at_unix_seconds,
            lease_nonce,
        );
        self.records[index].state = ApprovalState::Approved;
        self.append_event(AuditEventKind::ApprovalDecision {
            approval_id: id.to_owned(),
            actor: actor.to_owned(),
            outcome: ApprovalOutcome::Approved,
        })?;
        Ok(claims)
    }

    /// Rejects a pending approval.
    pub fn reject(
        &mut self,
        id: &str,
        actor: &str,
        now_unix_seconds: u64,
    ) -> Result<(), ApprovalError> {
        let index = self.index_of(id)?;
        let record = self.records[index].clone();
        self.ensure_pending_not_stale(&record, now_unix_seconds)?;
        self.records[index].state = ApprovalState::Rejected;
        self.append_event(AuditEventKind::ApprovalDecision {
            approval_id: id.to_owned(),
            actor: actor.to_owned(),
            outcome: ApprovalOutcome::Rejected,
        })
    }

    /// Marks a pending approval expired.
    pub fn expire(&mut self, id: &str, now_unix_seconds: u64) -> Result<(), ApprovalError> {
        let index = self.index_of(id)?;
        if self.records[index].state != ApprovalState::Pending {
            return Err(ApprovalError::NotPending);
        }
        if self.records[index].expires_at_unix_seconds > now_unix_seconds {
            return Err(ApprovalError::StaleApproval);
        }
        self.records[index].state = ApprovalState::Expired;
        self.append_event(AuditEventKind::ApprovalExpired {
            approval_id: id.to_owned(),
        })
    }

    /// Supersedes a pending approval with another approval id.
    pub fn supersede(&mut self, id: &str, superseded_by: &str) -> Result<(), ApprovalError> {
        let index = self.index_of(id)?;
        if self.records[index].state != ApprovalState::Pending {
            return Err(ApprovalError::NotPending);
        }
        self.records[index].state = ApprovalState::Superseded;
        self.append_event(AuditEventKind::ApprovalSuperseded {
            approval_id: id.to_owned(),
            superseded_by: superseded_by.to_owned(),
        })
    }

    /// Records an approval adapter rejection before it can mutate approval state.
    pub fn record_adapter_rejection(
        &mut self,
        adapter: &str,
        reason: &str,
    ) -> Result<(), ApprovalError> {
        self.append_event(AuditEventKind::ApprovalAdapterRejected {
            adapter: adapter.to_owned(),
            reason: reason.to_owned(),
        })
    }

    fn index_of(&self, id: &str) -> Result<usize, ApprovalError> {
        self.records
            .iter()
            .position(|record| record.id == id)
            .ok_or(ApprovalError::UnknownApproval)
    }

    fn ensure_pending_not_stale(
        &self,
        record: &ApprovalRecord,
        now_unix_seconds: u64,
    ) -> Result<(), ApprovalError> {
        if record.state != ApprovalState::Pending {
            return Err(ApprovalError::NotPending);
        }
        if record.expires_at_unix_seconds <= now_unix_seconds {
            return Err(ApprovalError::StaleApproval);
        }
        Ok(())
    }

    fn append_event(&mut self, kind: AuditEventKind) -> Result<(), ApprovalError> {
        let sequence = self.ledger.next_sequence();
        self.ledger
            .append(AuditEvent::new(
                format!("approval-event-{sequence}"),
                "approval",
                sequence,
                kind,
            ))
            .map_err(ApprovalError::AuditAppend)
    }
}

/// Creates a deterministic pending approval used by CLI demos and E2E tests.
#[must_use]
pub fn demo_approval_store() -> ApprovalStore {
    let proposal = crate::analyzer::analyze_service_unhealthy(
        "proposal-demo-1",
        "prod-web-01",
        "sshd",
        &[crate::EvidenceEnvelope::text(
            "service_status",
            "service=not-active",
        )],
    );
    let service = "system:node/prod-web-01/service/sshd".to_owned();
    let impact = ImpactSet::writes(OperationalLayer::System, [service.clone()])
        .with_may_affect([
            "platform:on-node/prod-web-01".to_owned(),
            "application:on-node/prod-web-01".to_owned(),
        ])
        .with_does_not_affect([
            "system:node/prod-web-01/package-db".to_owned(),
            "system:node/prod-web-01/firewall".to_owned(),
        ]);
    let verification = VerificationPlan::required([crate::VerificationCheck::new(
        "service_active",
        service,
        crate::VerificationTier::DirectImpact,
    )])
    .with_skipped([
        SkippedVerification::new(
            "package_audit",
            "service restart did not mutate package database",
        ),
        SkippedVerification::new(
            "firewall_audit",
            "service restart did not mutate firewall rules",
        ),
    ]);
    let mut store = ApprovalStore::empty();
    store
        .request(ApprovalRecord::new(
            "approval-demo-1",
            "run-demo-1",
            &proposal,
            "restart-service",
            OperationalLayer::System,
            "sshd",
            impact,
            verification,
            100,
            200,
        ))
        .expect("demo approval request is valid");
    store
}

fn node_from_target(resource_id: &str) -> String {
    resource_id
        .strip_prefix("system:node/")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("unknown-node")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{ApprovalError, ApprovalState, demo_approval_store};

    #[test]
    fn list_show_and_approve_bind_exact_action_to_lease_claims() {
        let mut store = demo_approval_store();
        assert_eq!(store.list_pending().len(), 1);
        let shown = store.show("approval-demo-1").expect("demo approval exists");
        assert_eq!(shown.action_id, "restart-service");
        assert_eq!(shown.layer, runlane_layer());
        assert!(shown.verification.skipped_checks_have_reasons());

        let claims = store
            .approve(
                "approval-demo-1",
                "restart-service",
                "operator",
                150,
                "allow-prod-web-sshd-restart",
                "lease-nonce",
            )
            .expect("approval succeeds");

        assert_eq!(claims.action, crate::ActionKind::ServiceRestart);
        assert_eq!(
            claims.target.resource_id,
            "system:node/prod-web-01/service/sshd"
        );
        assert_eq!(
            store.show("approval-demo-1").unwrap().state,
            ApprovalState::Approved
        );
    }

    #[test]
    fn reject_is_explicit_and_audited() {
        let mut store = demo_approval_store();
        store
            .reject("approval-demo-1", "operator", 150)
            .expect("reject succeeds");
        assert_eq!(
            store.show("approval-demo-1").unwrap().state,
            ApprovalState::Rejected
        );
        assert!(store.ledger.events().iter().any(|event| {
            matches!(
                event.kind,
                crate::AuditEventKind::ApprovalDecision {
                    outcome: crate::ApprovalOutcome::Rejected,
                    ..
                }
            )
        }));
    }

    #[test]
    fn stale_mismatched_and_superseded_approvals_fail_closed() {
        let mut stale = demo_approval_store();
        assert_eq!(
            stale
                .approve(
                    "approval-demo-1",
                    "restart-service",
                    "operator",
                    200,
                    "allow",
                    "nonce",
                )
                .expect_err("stale approval denied"),
            ApprovalError::StaleApproval
        );

        let mut mismatched = demo_approval_store();
        assert_eq!(
            mismatched
                .approve(
                    "approval-demo-1",
                    "other-action",
                    "operator",
                    150,
                    "allow",
                    "nonce",
                )
                .expect_err("action substitution denied"),
            ApprovalError::ActionMismatch
        );

        let mut superseded = demo_approval_store();
        superseded
            .supersede("approval-demo-1", "approval-demo-2")
            .expect("supersede succeeds");
        assert_eq!(
            superseded
                .reject("approval-demo-1", "operator", 150)
                .expect_err("superseded approval denied"),
            ApprovalError::NotPending
        );
    }

    fn runlane_layer() -> crate::OperationalLayer {
        crate::OperationalLayer::System
    }
}
