//! Shared domain vocabulary for Runlane.
//!
//! This crate intentionally contains no network, database, or OS-specific code.

/// Operational layer of a resource, task, runbook, or policy rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationalLayer {
    /// OS, kernel, system packages, users, privilege, firewall, filesystems, service manager.
    System,
    /// Databases, middleware, gateways, queues, caches, observability, shared platform services.
    Platform,
    /// Business applications, bots, workers, app configs, release artifacts.
    Application,
}

/// Operating systems supported as first-class agent targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatingSystem {
    Linux,
    FreeBsd,
    OpenBsd,
    Solaris,
    Illumos,
    Unknown,
}

/// High-level lifecycle for an incident run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunState {
    Created,
    Planned,
    CollectingEvidence,
    EvidenceCollected,
    ProposalGenerated,
    WaitingForApproval,
    Approved,
    Rejected,
    Executing,
    Verifying,
    Resolved,
    Failed,
    Escalated,
    Reviewed,
}

/// A capability reported by an agent or required by a runbook step.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Capability(pub String);

impl Capability {
    /// Creates a new capability identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Typed action names keep model output away from raw shell execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionKind {
    ServiceRestart,
    ServiceReload,
    RunAllowlistedScript,
    RemoveAllowlistedFile,
}

/// Evidence is data collected from a node. It is never executable instruction text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceEnvelope {
    pub source: String,
    pub content_type: String,
    pub body: String,
    pub truncated: bool,
}

impl EvidenceEnvelope {
    /// Creates a text evidence envelope.
    #[must_use]
    pub fn text(source: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            content_type: "text/plain".to_owned(),
            body: body.into(),
            truncated: false,
        }
    }
}

/// Returns true when a state transition is allowed by the initial v0.1 lifecycle.
#[must_use]
pub fn is_valid_run_transition(from: RunState, to: RunState) -> bool {
    use RunState::{
        Approved, CollectingEvidence, Created, Escalated, EvidenceCollected, Executing, Failed,
        Planned, ProposalGenerated, Rejected, Resolved, Reviewed, Verifying, WaitingForApproval,
    };

    matches!(
        (from, to),
        (Created, Planned)
            | (Planned, CollectingEvidence)
            | (CollectingEvidence, EvidenceCollected)
            | (EvidenceCollected, ProposalGenerated)
            | (ProposalGenerated, WaitingForApproval)
            | (WaitingForApproval, Approved | Rejected)
            | (Approved, Executing)
            | (Executing, Verifying)
            | (Verifying, Resolved | Failed | Escalated)
            | (Resolved | Failed | Escalated | Rejected, Reviewed)
    )
}

#[cfg(test)]
mod tests {
    use super::{OperationalLayer, RunState, is_valid_run_transition};

    #[test]
    fn allows_happy_path_transitions() {
        assert!(is_valid_run_transition(
            RunState::Created,
            RunState::Planned
        ));
        assert!(is_valid_run_transition(
            RunState::WaitingForApproval,
            RunState::Approved
        ));
        assert!(is_valid_run_transition(
            RunState::Verifying,
            RunState::Resolved
        ));
    }

    #[test]
    fn rejects_skipping_approval() {
        assert!(!is_valid_run_transition(
            RunState::ProposalGenerated,
            RunState::Executing
        ));
    }

    #[test]
    fn represents_all_operational_layers() {
        let layers = [
            OperationalLayer::System,
            OperationalLayer::Platform,
            OperationalLayer::Application,
        ];
        assert_eq!(layers.len(), 3);
    }
}
