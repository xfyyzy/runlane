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

/// Technical shape of a resource.
///
/// The kind is intentionally separate from [`OperationalLayer`]. For example,
/// a service may be system, platform, or application depending on its
/// operational role.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Node,
    Service,
    ServiceManager,
    Logs,
    ProcessSet,
    Socket,
    Port,
    Filesystem,
    Mount,
    Disk,
    PackageDb,
    Firewall,
    Route,
    User,
    Group,
    PrivilegeRule,
    ScheduledJob,
    KernelTunable,
    Reboot,
    Database,
    Gateway,
    Queue,
    Cache,
    Observability,
    Application,
    Worker,
    Endpoint,
    Certificate,
    Custom(String),
}

/// Ownership or selection scope for a resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceScope {
    Fleet,
    Environment(String),
    Role(String),
    NodeGroup(String),
    Node(String),
    PlatformInstance(String),
    Application(String),
    Global,
}

/// A typed resource that tasks may read, write, affect, or lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub id: String,
    pub layer: OperationalLayer,
    pub kind: ResourceKind,
    pub scope: ResourceScope,
    pub depends_on: Vec<String>,
}

impl Resource {
    /// Creates a new resource with no dependencies.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        layer: OperationalLayer,
        kind: ResourceKind,
        scope: ResourceScope,
    ) -> Self {
        Self {
            id: id.into(),
            layer,
            kind,
            scope,
            depends_on: Vec::new(),
        }
    }

    /// Adds dependency resource ids to the resource.
    #[must_use]
    pub fn with_dependencies(mut self, depends_on: impl IntoIterator<Item = String>) -> Self {
        self.depends_on = depends_on.into_iter().collect();
        self
    }
}

/// Lease mode requested or held for a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeaseMode {
    Observe,
    Intent,
    Exclusive,
    Drain,
    Reboot,
}

impl LeaseMode {
    /// Returns true when the lease mode can create side effects.
    #[must_use]
    pub const fn is_mutating(self) -> bool {
        matches!(self, Self::Exclusive | Self::Drain | Self::Reboot)
    }

    /// Returns true when two same-resource lease modes can coexist.
    #[must_use]
    pub const fn is_compatible_with(self, other: Self) -> bool {
        use LeaseMode::{Drain, Exclusive, Intent, Observe, Reboot};

        matches!(
            (self, other),
            (Observe, Observe | Intent | Exclusive | Drain)
                | (Intent, Observe | Intent | Exclusive)
                | (Exclusive, Observe | Intent)
                | (Drain, Observe | Reboot)
                | (Reboot, Drain)
        )
    }
}

/// Temporary right for a run or task to observe, plan, or mutate a resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLease {
    pub id: String,
    pub run_id: String,
    pub task_id: Option<String>,
    pub resource_id: String,
    pub mode: LeaseMode,
    pub reason: String,
}

impl ResourceLease {
    /// Creates a run-scoped resource lease.
    #[must_use]
    pub fn for_run(
        id: impl Into<String>,
        run_id: impl Into<String>,
        resource_id: impl Into<String>,
        mode: LeaseMode,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            run_id: run_id.into(),
            task_id: None,
            resource_id: resource_id.into(),
            mode,
            reason: reason.into(),
        }
    }

    /// Attaches the lease to a task inside the run.
    #[must_use]
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }
}

/// Resources affected by a task or run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactSet {
    pub layer: OperationalLayer,
    pub writes: Vec<String>,
    pub may_affect: Vec<String>,
    pub does_not_affect: Vec<String>,
}

impl ImpactSet {
    /// Creates an empty impact set for a layer.
    #[must_use]
    pub const fn empty(layer: OperationalLayer) -> Self {
        Self {
            layer,
            writes: Vec::new(),
            may_affect: Vec::new(),
            does_not_affect: Vec::new(),
        }
    }

    /// Creates an impact set with direct writes.
    #[must_use]
    pub fn writes(layer: OperationalLayer, writes: impl IntoIterator<Item = String>) -> Self {
        Self {
            layer,
            writes: writes.into_iter().collect(),
            may_affect: Vec::new(),
            does_not_affect: Vec::new(),
        }
    }

    /// Adds indirect impact resources.
    #[must_use]
    pub fn with_may_affect(mut self, resources: impl IntoIterator<Item = String>) -> Self {
        self.may_affect = resources.into_iter().collect();
        self
    }

    /// Adds resources explicitly excluded from impact.
    #[must_use]
    pub fn with_does_not_affect(mut self, resources: impl IntoIterator<Item = String>) -> Self {
        self.does_not_affect = resources.into_iter().collect();
        self
    }
}

/// Cost and breadth tier for a verification check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerificationTier {
    Precondition,
    DirectImpact,
    Dependent,
    BroadRegression,
}

/// A verification check selected for a run or task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationCheck {
    pub id: String,
    pub resource_id: String,
    pub tier: VerificationTier,
}

impl VerificationCheck {
    /// Creates a verification check.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        resource_id: impl Into<String>,
        tier: VerificationTier,
    ) -> Self {
        Self {
            id: id.into(),
            resource_id: resource_id.into(),
            tier,
        }
    }
}

/// A verification check intentionally skipped with an audit-ready reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedVerification {
    pub check_id: String,
    pub reason: String,
}

impl SkippedVerification {
    /// Creates a skipped verification entry.
    #[must_use]
    pub fn new(check_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            check_id: check_id.into(),
            reason: reason.into(),
        }
    }
}

/// Verification plan selected from layer, impact, and policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPlan {
    pub required: Vec<VerificationCheck>,
    pub conditional: Vec<VerificationCheck>,
    pub skipped: Vec<SkippedVerification>,
}

impl VerificationPlan {
    /// Creates an empty verification plan.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            required: Vec::new(),
            conditional: Vec::new(),
            skipped: Vec::new(),
        }
    }

    /// Creates a plan with required checks.
    #[must_use]
    pub fn required(checks: impl IntoIterator<Item = VerificationCheck>) -> Self {
        Self {
            required: checks.into_iter().collect(),
            conditional: Vec::new(),
            skipped: Vec::new(),
        }
    }

    /// Adds skipped checks with audit reasons.
    #[must_use]
    pub fn with_skipped(mut self, skipped: impl IntoIterator<Item = SkippedVerification>) -> Self {
        self.skipped = skipped.into_iter().collect();
        self
    }
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

/// A schedulable unit inside a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub layer: OperationalLayer,
    pub required_capabilities: Vec<Capability>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub dependencies: Vec<String>,
    pub impact: ImpactSet,
    pub verification: VerificationPlan,
}

impl Task {
    /// Creates a task with no resources or dependencies.
    #[must_use]
    pub fn new(id: impl Into<String>, layer: OperationalLayer) -> Self {
        Self {
            id: id.into(),
            layer,
            required_capabilities: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            dependencies: Vec::new(),
            impact: ImpactSet::empty(layer),
            verification: VerificationPlan::empty(),
        }
    }

    /// Adds required capability identifiers.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: impl IntoIterator<Item = Capability>) -> Self {
        self.required_capabilities = capabilities.into_iter().collect();
        self
    }

    /// Adds resources read by the task.
    #[must_use]
    pub fn with_reads(mut self, reads: impl IntoIterator<Item = String>) -> Self {
        self.reads = reads.into_iter().collect();
        self
    }

    /// Adds resources written by the task.
    #[must_use]
    pub fn with_writes(mut self, writes: impl IntoIterator<Item = String>) -> Self {
        self.writes = writes.into_iter().collect();
        self
    }

    /// Adds task dependency ids.
    #[must_use]
    pub fn with_dependencies(mut self, dependencies: impl IntoIterator<Item = String>) -> Self {
        self.dependencies = dependencies.into_iter().collect();
        self
    }

    /// Sets the task impact.
    #[must_use]
    pub fn with_impact(mut self, impact: ImpactSet) -> Self {
        self.impact = impact;
        self
    }

    /// Sets the task verification plan.
    #[must_use]
    pub fn with_verification(mut self, verification: VerificationPlan) -> Self {
        self.verification = verification;
        self
    }
}

/// One execution of a runbook or manual operational task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub id: String,
    pub goal: String,
    pub target_layer: OperationalLayer,
    pub state: RunState,
    pub tasks: Vec<Task>,
    pub leases: Vec<ResourceLease>,
    pub impact: ImpactSet,
    pub verification: VerificationPlan,
}

impl Run {
    /// Creates a run in the created state.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        goal: impl Into<String>,
        target_layer: OperationalLayer,
    ) -> Self {
        Self {
            id: id.into(),
            goal: goal.into(),
            target_layer,
            state: RunState::Created,
            tasks: Vec::new(),
            leases: Vec::new(),
            impact: ImpactSet::empty(target_layer),
            verification: VerificationPlan::empty(),
        }
    }

    /// Attempts to transition the run to another state.
    pub fn transition_to(&mut self, to: RunState) -> Result<(), InvalidRunTransition> {
        if is_valid_run_transition(self.state, to) {
            self.state = to;
            Ok(())
        } else {
            Err(InvalidRunTransition {
                from: self.state,
                to,
            })
        }
    }

    /// Adds tasks to the run.
    #[must_use]
    pub fn with_tasks(mut self, tasks: impl IntoIterator<Item = Task>) -> Self {
        self.tasks = tasks.into_iter().collect();
        self
    }

    /// Adds leases to the run.
    #[must_use]
    pub fn with_leases(mut self, leases: impl IntoIterator<Item = ResourceLease>) -> Self {
        self.leases = leases.into_iter().collect();
        self
    }
}

/// Error returned when a run state transition is not allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRunTransition {
    pub from: RunState,
    pub to: RunState,
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
    use super::{
        Capability, ImpactSet, LeaseMode, OperationalLayer, Resource, ResourceKind, ResourceLease,
        ResourceScope, Run, RunState, SkippedVerification, Task, VerificationCheck,
        VerificationPlan, VerificationTier, is_valid_run_transition,
    };

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
        let resources = [
            Resource::new(
                "system:node/prod-web-01/package-db",
                OperationalLayer::System,
                ResourceKind::PackageDb,
                ResourceScope::Node("prod-web-01".to_owned()),
            ),
            Resource::new(
                "platform:postgres/main",
                OperationalLayer::Platform,
                ResourceKind::Database,
                ResourceScope::PlatformInstance("postgres/main".to_owned()),
            )
            .with_dependencies([
                "system:node/db-01/filesystem/var-lib-postgresql".to_owned(),
                "system:node/db-02/filesystem/var-lib-postgresql".to_owned(),
            ]),
            Resource::new(
                "application:sports/api-gateway",
                OperationalLayer::Application,
                ResourceKind::Application,
                ResourceScope::Application("sports/api-gateway".to_owned()),
            )
            .with_dependencies([
                "platform:postgres/main".to_owned(),
                "platform:redis/cache".to_owned(),
            ]),
        ];

        let layers = resources.map(|resource| resource.layer);
        assert_eq!(
            layers,
            [
                OperationalLayer::System,
                OperationalLayer::Platform,
                OperationalLayer::Application,
            ]
        );
    }

    #[test]
    fn models_task_impact_leases_and_verification() {
        let service_id = "system:node/prod-web-01/service/sshd".to_owned();
        let task = Task::new("restart-sshd", OperationalLayer::System)
            .with_capabilities([Capability::new("service.systemd")])
            .with_reads([
                service_id.clone(),
                "system:node/prod-web-01/logs/sshd".to_owned(),
            ])
            .with_writes([service_id.clone()])
            .with_impact(
                ImpactSet::writes(OperationalLayer::System, [service_id.clone()])
                    .with_may_affect([
                        "platform:on-node/prod-web-01".to_owned(),
                        "application:on-node/prod-web-01".to_owned(),
                    ])
                    .with_does_not_affect([
                        "system:node/prod-web-01/package-db".to_owned(),
                        "system:node/prod-web-01/firewall".to_owned(),
                    ]),
            )
            .with_verification(
                VerificationPlan::required([VerificationCheck::new(
                    "service_active",
                    service_id.clone(),
                    VerificationTier::DirectImpact,
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
                ]),
            );

        let lease = ResourceLease::for_run(
            "lease-1",
            "run-1",
            service_id,
            LeaseMode::Exclusive,
            "approved service restart",
        )
        .with_task(task.id.clone());

        let run = Run::new("run-1", "restart unhealthy sshd", OperationalLayer::System)
            .with_tasks([task])
            .with_leases([lease]);

        assert_eq!(run.tasks.len(), 1);
        assert_eq!(run.leases[0].mode, LeaseMode::Exclusive);
        assert!(run.leases[0].mode.is_mutating());
        assert_eq!(run.tasks[0].verification.skipped.len(), 2);
    }

    #[test]
    fn run_transition_method_rejects_invalid_transition() {
        let mut run = Run::new("run-1", "invalid skip", OperationalLayer::System);

        let err = run
            .transition_to(RunState::Executing)
            .expect_err("created runs cannot execute without planning and approval");

        assert_eq!(err.from, RunState::Created);
        assert_eq!(err.to, RunState::Executing);
        assert_eq!(run.state, RunState::Created);
    }

    #[test]
    fn lease_modes_express_basic_same_resource_compatibility() {
        assert!(LeaseMode::Observe.is_compatible_with(LeaseMode::Intent));
        assert!(LeaseMode::Intent.is_compatible_with(LeaseMode::Exclusive));
        assert!(!LeaseMode::Exclusive.is_compatible_with(LeaseMode::Exclusive));
        assert!(!LeaseMode::Observe.is_compatible_with(LeaseMode::Reboot));
    }
}
