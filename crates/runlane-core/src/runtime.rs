use crate::{
    AgentIdentity, AgentProtocolContext, AgentResultSubmission, AgentTaskEnvelope,
    AgentTaskRejection, AuditAppendError, AuditEvent, AuditEventKind, AuditLedger, Capability,
    EvidenceEnvelope, LocalSpoolItem, OperatingSystem, validate_agent_task_envelope,
};

/// Enrollment token issued by the server for one expected node identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollmentToken {
    pub token_id: String,
    pub token: String,
    pub node_id: String,
    pub os: OperatingSystem,
    pub server_trust_root: String,
    pub expires_at_unix_seconds: u64,
    pub nonce: String,
}

impl EnrollmentToken {
    /// Creates an enrollment token.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        token_id: impl Into<String>,
        token: impl Into<String>,
        node_id: impl Into<String>,
        os: OperatingSystem,
        server_trust_root: impl Into<String>,
        expires_at_unix_seconds: u64,
        nonce: impl Into<String>,
    ) -> Self {
        Self {
            token_id: token_id.into(),
            token: token.into(),
            node_id: node_id.into(),
            os,
            server_trust_root: server_trust_root.into(),
            expires_at_unix_seconds,
            nonce: nonce.into(),
        }
    }
}

/// Agent enrollment request submitted through the control-plane boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentEnrollmentRequest {
    pub token: String,
    pub node_id: String,
    pub os: OperatingSystem,
    pub certificate_fingerprint: String,
    pub server_trust_root: String,
    pub now_unix_seconds: u64,
}

impl AgentEnrollmentRequest {
    /// Creates an enrollment request.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        token: impl Into<String>,
        node_id: impl Into<String>,
        os: OperatingSystem,
        certificate_fingerprint: impl Into<String>,
        server_trust_root: impl Into<String>,
        now_unix_seconds: u64,
    ) -> Self {
        Self {
            token: token.into(),
            node_id: node_id.into(),
            os,
            certificate_fingerprint: certificate_fingerprint.into(),
            server_trust_root: server_trust_root.into(),
            now_unix_seconds,
        }
    }
}

/// Durable-enough local agent identity metadata for development and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentityRecord {
    pub node_id: String,
    pub os: OperatingSystem,
    pub certificate_fingerprint: String,
    pub server_trust_root: String,
}

impl AgentIdentityRecord {
    /// Returns the protocol identity for an enrolled agent.
    #[must_use]
    pub fn identity(&self) -> AgentIdentity {
        AgentIdentity::new(&self.node_id, &self.certificate_fingerprint)
    }
}

/// Pending typed task stored by the server before an agent pulls it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAgentTask {
    pub envelope: AgentTaskEnvelope,
    pub payload: TypedTaskPayload,
}

impl PendingAgentTask {
    /// Creates a pending typed task.
    #[must_use]
    pub fn new(envelope: AgentTaskEnvelope, payload: TypedTaskPayload) -> Self {
        Self { envelope, payload }
    }
}

/// Task payload that cannot contain executable shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedTaskPayload {
    CollectEvidence {
        capability: Capability,
        resource_id: String,
    },
    ExecuteTypedHelperRequest {
        action: String,
        target_resource_id: String,
    },
    Verify {
        check_id: String,
        resource_id: String,
    },
}

/// Accepted pulled task including typed payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulledAgentTask {
    pub envelope: AgentTaskEnvelope,
    pub payload: TypedTaskPayload,
}

/// Runtime API failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    ExpiredEnrollmentToken,
    EnrollmentTokenMismatch,
    EnrollmentNodeMismatch,
    EnrollmentOsMismatch,
    EnrollmentTrustRootMismatch,
    UnknownAgent,
    AgentIdentityMismatch,
    NoTaskAvailable,
    TaskEnvelopeRejected(AgentTaskRejection),
    UnknownEnvelope,
    ResultEnvelopeMismatch,
    ResultTaskMismatch,
    ResultRunMismatch,
    ResultNodeMismatch,
    ResultExpired,
    ResultNonceReplay,
    AuditAppend(AuditAppendError),
}

/// In-process control-plane boundary used by server, agent, and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPlane {
    enrollment_tokens: Vec<EnrollmentToken>,
    agents: Vec<AgentIdentityRecord>,
    pending_tasks: Vec<PendingAgentTask>,
    pulled_tasks: Vec<PendingAgentTask>,
    accepted_result_nonces: Vec<String>,
    pub accepted_results: Vec<AgentResultSubmission>,
    pub spooled_results: Vec<LocalSpoolItem>,
    pub ledger: AuditLedger,
}

impl ControlPlane {
    /// Creates an empty in-process control plane.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            enrollment_tokens: Vec::new(),
            agents: Vec::new(),
            pending_tasks: Vec::new(),
            pulled_tasks: Vec::new(),
            accepted_result_nonces: Vec::new(),
            accepted_results: Vec::new(),
            spooled_results: Vec::new(),
            ledger: AuditLedger::empty(),
        }
    }

    /// Issues an enrollment token and records an audit event.
    pub fn create_enrollment_token(&mut self, token: EnrollmentToken) -> Result<(), RuntimeError> {
        self.append_event(AuditEventKind::EnrollmentTokenCreated {
            token_id: token.token_id.clone(),
            node_id: token.node_id.clone(),
        })?;
        self.enrollment_tokens.push(token);
        Ok(())
    }

    /// Enrolls an agent if token, node, OS, trust root, and expiry match.
    pub fn enroll_agent(
        &mut self,
        request: &AgentEnrollmentRequest,
    ) -> Result<AgentIdentityRecord, RuntimeError> {
        let token = self
            .enrollment_tokens
            .iter()
            .find(|token| token.token == request.token)
            .ok_or(RuntimeError::EnrollmentTokenMismatch)?;

        if token.expires_at_unix_seconds <= request.now_unix_seconds {
            return Err(RuntimeError::ExpiredEnrollmentToken);
        }
        if token.node_id != request.node_id {
            return Err(RuntimeError::EnrollmentNodeMismatch);
        }
        if token.os != request.os {
            return Err(RuntimeError::EnrollmentOsMismatch);
        }
        if token.server_trust_root != request.server_trust_root {
            return Err(RuntimeError::EnrollmentTrustRootMismatch);
        }

        let record = AgentIdentityRecord {
            node_id: request.node_id.clone(),
            os: request.os,
            certificate_fingerprint: request.certificate_fingerprint.clone(),
            server_trust_root: request.server_trust_root.clone(),
        };
        self.append_event(AuditEventKind::AgentEnrolled {
            node_id: record.node_id.clone(),
            os: record.os,
        })?;
        self.agents.push(record.clone());
        Ok(record)
    }

    /// Queues a typed task for a node.
    pub fn enqueue_task(&mut self, task: PendingAgentTask) {
        self.pending_tasks.push(task);
    }

    /// Pulls the next task for an enrolled agent.
    pub fn pull_task(
        &mut self,
        identity: &AgentIdentity,
        now_unix_seconds: u64,
    ) -> Result<PulledAgentTask, RuntimeError> {
        self.require_agent(identity)?;
        let index = self
            .pending_tasks
            .iter()
            .position(|task| task.envelope.node_id == identity.node_id)
            .ok_or(RuntimeError::NoTaskAvailable)?;
        let task = self.pending_tasks[index].clone();
        let context = AgentProtocolContext::new(&identity.node_id, now_unix_seconds, []);
        if let Err(rejection) = validate_agent_task_envelope(&task.envelope, &context) {
            self.append_event(AuditEventKind::AgentTaskRejected {
                node_id: identity.node_id.clone(),
                reason: format!("{rejection:?}"),
            })?;
            return Err(RuntimeError::TaskEnvelopeRejected(rejection));
        }
        let task = self.pending_tasks.remove(index);

        self.append_event(AuditEventKind::AgentTaskPulled {
            envelope_id: task.envelope.envelope_id.clone(),
            task_id: task.envelope.task_id.clone(),
            node_id: task.envelope.node_id.clone(),
        })?;
        self.pulled_tasks.push(task.clone());
        Ok(PulledAgentTask {
            envelope: task.envelope,
            payload: task.payload,
        })
    }

    /// Submits structured task results for a previously pulled envelope.
    pub fn submit_result(
        &mut self,
        identity: &AgentIdentity,
        submission: AgentResultSubmission,
        now_unix_seconds: u64,
    ) -> Result<(), RuntimeError> {
        self.require_agent(identity)?;
        if submission.node_id != identity.node_id {
            self.audit_result_rejection(&submission, "result node mismatch")?;
            return Err(RuntimeError::ResultNodeMismatch);
        }
        if self
            .accepted_result_nonces
            .iter()
            .any(|nonce| nonce == &submission.nonce)
        {
            self.audit_result_rejection(&submission, "result nonce replay")?;
            return Err(RuntimeError::ResultNonceReplay);
        }
        let task = self
            .pulled_tasks
            .iter()
            .find(|task| task.envelope.envelope_id == submission.envelope_id)
            .ok_or(RuntimeError::UnknownEnvelope)?;
        if task.envelope.expires_at_unix_seconds <= now_unix_seconds {
            self.audit_result_rejection(&submission, "result envelope expired")?;
            return Err(RuntimeError::ResultExpired);
        }
        if task.envelope.task_id != submission.task_id {
            self.audit_result_rejection(&submission, "result task mismatch")?;
            return Err(RuntimeError::ResultTaskMismatch);
        }
        if task.envelope.run_id != submission.run_id {
            self.audit_result_rejection(&submission, "result run mismatch")?;
            return Err(RuntimeError::ResultRunMismatch);
        }
        if task.envelope.nonce != submission.nonce {
            self.audit_result_rejection(&submission, "result nonce mismatch")?;
            return Err(RuntimeError::ResultEnvelopeMismatch);
        }

        self.append_event(AuditEventKind::AgentResultAccepted {
            envelope_id: submission.envelope_id.clone(),
            task_id: submission.task_id.clone(),
            node_id: submission.node_id.clone(),
        })?;
        self.accepted_result_nonces.push(submission.nonce.clone());
        self.accepted_results.push(submission);
        Ok(())
    }

    /// Preserves a failed result submission as a local spool item.
    pub fn spool_result(&mut self, spool: LocalSpoolItem) -> Result<LocalSpoolItem, RuntimeError> {
        self.append_event(AuditEventKind::AgentResultSpooled {
            spool_id: spool.spool_id.clone(),
            reason: spool.reason.clone(),
        })?;
        self.spooled_results.push(spool.clone());
        Ok(spool)
    }

    /// Returns enrolled agent records.
    #[must_use]
    pub fn agents(&self) -> &[AgentIdentityRecord] {
        &self.agents
    }

    fn require_agent(&self, identity: &AgentIdentity) -> Result<(), RuntimeError> {
        let agent = self
            .agents
            .iter()
            .find(|agent| agent.node_id == identity.node_id)
            .ok_or(RuntimeError::UnknownAgent)?;
        if agent.certificate_fingerprint == identity.certificate_fingerprint {
            Ok(())
        } else {
            Err(RuntimeError::AgentIdentityMismatch)
        }
    }

    fn audit_result_rejection(
        &mut self,
        submission: &AgentResultSubmission,
        reason: &str,
    ) -> Result<(), RuntimeError> {
        self.append_event(AuditEventKind::AgentResultRejected {
            envelope_id: submission.envelope_id.clone(),
            reason: reason.to_owned(),
        })
    }

    fn append_event(&mut self, kind: AuditEventKind) -> Result<(), RuntimeError> {
        let sequence = self.ledger.next_sequence();
        self.ledger
            .append(AuditEvent::new(
                format!("runtime-event-{sequence}"),
                "runtime",
                sequence,
                kind,
            ))
            .map_err(RuntimeError::AuditAppend)
    }
}

/// Creates deterministic evidence for runtime tests and local simulation.
#[must_use]
pub fn runtime_text_evidence(source: &str, body: &str) -> EvidenceEnvelope {
    EvidenceEnvelope::text(source, body)
}

#[cfg(test)]
mod tests {
    use crate::{
        AgentResultStatus, AgentResultSubmission, Capability, LocalSpoolItem, OperatingSystem,
        SpoolReason,
    };

    use super::{
        AgentEnrollmentRequest, ControlPlane, EnrollmentToken, PendingAgentTask, RuntimeError,
        TypedTaskPayload, runtime_text_evidence,
    };
    use crate::AgentTaskEnvelope;

    #[test]
    fn enrolls_agent_pulls_typed_task_and_accepts_result() {
        let mut server = enrolled_server();
        let identity = server.agents()[0].identity();
        server.enqueue_task(test_task(110));

        let pulled = server
            .pull_task(&identity, 100)
            .expect("enrolled agent pulls task");
        assert!(matches!(
            pulled.payload,
            TypedTaskPayload::CollectEvidence { .. }
        ));

        server
            .submit_result(
                &identity,
                AgentResultSubmission::new(
                    "env-1",
                    "run-1",
                    "task-1",
                    "prod-web-01",
                    "nonce-1",
                    AgentResultStatus::Succeeded,
                    [runtime_text_evidence("service_status", "sshd active")],
                    "audit-1",
                ),
                105,
            )
            .expect("valid result accepted");

        assert_eq!(server.accepted_results.len(), 1);
        assert!(server.ledger.events().iter().any(|event| matches!(
            event.kind,
            crate::AuditEventKind::AgentResultAccepted { .. }
        )));
    }

    #[test]
    fn rejects_wrong_agent_identity_and_expired_pull() {
        let mut server = enrolled_server();
        let mut identity = server.agents()[0].identity();
        identity.certificate_fingerprint = "different".to_owned();
        server.enqueue_task(test_task(110));

        assert_eq!(
            server
                .pull_task(&identity, 100)
                .expect_err("wrong cert denied"),
            RuntimeError::AgentIdentityMismatch
        );

        let identity = server.agents()[0].identity();
        assert_eq!(
            server
                .pull_task(&identity, 111)
                .expect_err("expired task denied"),
            RuntimeError::TaskEnvelopeRejected(crate::AgentTaskRejection::ExpiredEnvelope)
        );
    }

    #[test]
    fn rejects_replayed_or_mismatched_result_and_spools_failed_submission() {
        let mut server = enrolled_server();
        let identity = server.agents()[0].identity();
        server.enqueue_task(test_task(120));
        server.pull_task(&identity, 100).expect("pull succeeds");

        let result = AgentResultSubmission::new(
            "env-1",
            "run-1",
            "task-1",
            "prod-web-01",
            "nonce-1",
            AgentResultStatus::Succeeded,
            [runtime_text_evidence("service_status", "sshd active")],
            "audit-1",
        );
        server
            .submit_result(&identity, result.clone(), 105)
            .expect("first result accepted");
        assert_eq!(
            server
                .submit_result(&identity, result.clone(), 106)
                .expect_err("replayed result denied"),
            RuntimeError::ResultNonceReplay
        );

        let spooled = server
            .spool_result(LocalSpoolItem::new(
                "spool-1",
                SpoolReason::ServerUnavailable,
                result,
            ))
            .expect("spool recorded");
        assert_eq!(spooled.spool_id, "spool-1");
        assert_eq!(server.spooled_results.len(), 1);
    }

    #[test]
    fn enrollment_fails_closed_for_expired_or_mismatched_token() {
        let mut server = ControlPlane::empty();
        server
            .create_enrollment_token(EnrollmentToken::new(
                "token-1",
                "secret",
                "prod-web-01",
                OperatingSystem::Linux,
                "trust-root",
                100,
                "enroll-nonce",
            ))
            .expect("token created");
        let expired = AgentEnrollmentRequest::new(
            "secret",
            "prod-web-01",
            OperatingSystem::Linux,
            "cert",
            "trust-root",
            101,
        );
        assert_eq!(
            server
                .enroll_agent(&expired)
                .expect_err("expired token denied"),
            RuntimeError::ExpiredEnrollmentToken
        );

        let wrong_node = AgentEnrollmentRequest::new(
            "secret",
            "other",
            OperatingSystem::Linux,
            "cert",
            "trust-root",
            99,
        );
        assert_eq!(
            server
                .enroll_agent(&wrong_node)
                .expect_err("wrong node denied"),
            RuntimeError::EnrollmentNodeMismatch
        );
    }

    fn enrolled_server() -> ControlPlane {
        let mut server = ControlPlane::empty();
        server
            .create_enrollment_token(EnrollmentToken::new(
                "token-1",
                "secret",
                "prod-web-01",
                OperatingSystem::Linux,
                "trust-root",
                200,
                "enroll-nonce",
            ))
            .expect("token created");
        server
            .enroll_agent(&AgentEnrollmentRequest::new(
                "secret",
                "prod-web-01",
                OperatingSystem::Linux,
                "cert-fingerprint",
                "trust-root",
                100,
            ))
            .expect("agent enrolled");
        server
    }

    fn test_task(expires_at_unix_seconds: u64) -> PendingAgentTask {
        PendingAgentTask::new(
            AgentTaskEnvelope::new(
                "env-1",
                "run-1",
                "task-1",
                "prod-web-01",
                100,
                expires_at_unix_seconds,
                "nonce-1",
                [Capability::new("service.systemd")],
                "audit-1",
            ),
            TypedTaskPayload::CollectEvidence {
                capability: Capability::new("service.systemd"),
                resource_id: "system:node/prod-web-01/service/sshd".to_owned(),
            },
        )
    }
}
