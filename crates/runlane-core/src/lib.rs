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

/// Lease a task requests before it can run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLeaseRequest {
    pub resource_id: String,
    pub mode: LeaseMode,
    pub reason: String,
}

impl ResourceLeaseRequest {
    /// Creates a resource lease request.
    #[must_use]
    pub fn new(resource_id: impl Into<String>, mode: LeaseMode, reason: impl Into<String>) -> Self {
        Self {
            resource_id: resource_id.into(),
            mode,
            reason: reason.into(),
        }
    }
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

    /// Returns the capability identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Capability reported as unsupported by a backend, with a fail-closed reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedCapability {
    pub capability: Capability,
    pub reason: String,
}

impl UnsupportedCapability {
    /// Creates an unsupported capability entry.
    #[must_use]
    pub fn new(capability: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            capability: Capability::new(capability),
            reason: reason.into(),
        }
    }
}

/// Native capability report submitted by an agent backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    pub node_id: String,
    pub os: OperatingSystem,
    pub capabilities: Vec<Capability>,
    pub unsupported: Vec<UnsupportedCapability>,
}

impl CapabilityReport {
    /// Creates a capability report.
    #[must_use]
    pub fn new(
        node_id: impl Into<String>,
        os: OperatingSystem,
        capabilities: impl IntoIterator<Item = Capability>,
        unsupported: impl IntoIterator<Item = UnsupportedCapability>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            os,
            capabilities: capabilities.into_iter().collect(),
            unsupported: unsupported.into_iter().collect(),
        }
    }

    /// Returns true if the backend reported a supported capability.
    #[must_use]
    pub fn supports(&self, capability: &Capability) -> bool {
        self.capabilities
            .iter()
            .any(|existing| existing == capability)
    }
}

/// Fail-closed capability error shape shared by platform backends and runbook planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityFailure {
    Unsupported(UnsupportedCapability),
    BackendUnavailable { os: OperatingSystem, reason: String },
}

/// Typed action names keep model output away from raw shell execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionKind {
    ServiceRestart,
    ServiceReload,
    RunAllowlistedScript,
    RemoveAllowlistedFile,
}

/// Target bound to a typed helper action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionTarget {
    pub resource_id: String,
    pub subject: String,
}

impl ActionTarget {
    /// Creates an action target bound to a resource id and local subject.
    #[must_use]
    pub fn new(resource_id: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            resource_id: resource_id.into(),
            subject: subject.into(),
        }
    }
}

/// Claims signed by the server before a helper can perform a privileged action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityLeaseClaims {
    pub lease_id: String,
    pub run_id: String,
    pub approval_id: String,
    pub node_id: String,
    pub action: ActionKind,
    pub target: ActionTarget,
    pub allowlist_entry_id: String,
    pub expires_at_unix_seconds: u64,
    pub nonce: String,
}

impl CapabilityLeaseClaims {
    /// Creates lease claims for a typed action target.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lease_id: impl Into<String>,
        run_id: impl Into<String>,
        approval_id: impl Into<String>,
        node_id: impl Into<String>,
        action: ActionKind,
        target: ActionTarget,
        allowlist_entry_id: impl Into<String>,
        expires_at_unix_seconds: u64,
        nonce: impl Into<String>,
    ) -> Self {
        Self {
            lease_id: lease_id.into(),
            run_id: run_id.into(),
            approval_id: approval_id.into(),
            node_id: node_id.into(),
            action,
            target,
            allowlist_entry_id: allowlist_entry_id.into(),
            expires_at_unix_seconds,
            nonce: nonce.into(),
        }
    }
}

/// Signed lease envelope. The signature bytes are opaque to the domain model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCapabilityLease {
    pub claims: CapabilityLeaseClaims,
    pub key_id: String,
    pub signature: String,
}

impl SignedCapabilityLease {
    /// Creates a signed lease envelope.
    #[must_use]
    pub fn new(
        claims: CapabilityLeaseClaims,
        key_id: impl Into<String>,
        signature: impl Into<String>,
    ) -> Self {
        Self {
            claims,
            key_id: key_id.into(),
            signature: signature.into(),
        }
    }
}

/// Signature verification status provided by the helper cryptographic layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseSignatureStatus {
    Valid,
    Invalid,
}

/// A local helper allowlist entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperAllowlistEntry {
    pub id: String,
    pub action: ActionKind,
    pub target_resource_id: String,
}

impl HelperAllowlistEntry {
    /// Creates a local helper allowlist entry.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        action: ActionKind,
        target_resource_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            action,
            target_resource_id: target_resource_id.into(),
        }
    }

    /// Returns true when the entry permits the action target.
    #[must_use]
    pub fn permits(&self, action: &ActionKind, target: &ActionTarget) -> bool {
        self.action == *action && self.target_resource_id == target.resource_id
    }
}

/// Local helper allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperAllowlist {
    pub entries: Vec<HelperAllowlistEntry>,
}

impl HelperAllowlist {
    /// Creates a helper allowlist.
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = HelperAllowlistEntry>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Returns true when the allowlist entry id permits the action target.
    #[must_use]
    pub fn permits(&self, entry_id: &str, action: &ActionKind, target: &ActionTarget) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.id == entry_id && entry.permits(action, target))
    }
}

/// Key-value action argument for typed helper requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperArgument {
    pub name: String,
    pub value: String,
}

impl HelperArgument {
    /// Creates a helper argument.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Typed helper action request. It contains no shell command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperActionRequest {
    pub lease_id: String,
    pub action: ActionKind,
    pub target: ActionTarget,
    pub arguments: Vec<HelperArgument>,
}

impl HelperActionRequest {
    /// Creates a helper action request.
    #[must_use]
    pub fn new(
        lease_id: impl Into<String>,
        action: ActionKind,
        target: ActionTarget,
        arguments: impl IntoIterator<Item = HelperArgument>,
    ) -> Self {
        Self {
            lease_id: lease_id.into(),
            action,
            target,
            arguments: arguments.into_iter().collect(),
        }
    }
}

/// Helper action result status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperActionStatus {
    Succeeded,
    Failed,
    Denied,
}

/// Structured helper response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperActionResponse {
    pub status: HelperActionStatus,
    pub message: String,
}

impl HelperActionResponse {
    /// Creates a helper response.
    #[must_use]
    pub fn new(status: HelperActionStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

/// Helper-side validation context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperValidationContext {
    pub node_id: String,
    pub now_unix_seconds: u64,
    pub signature_status: LeaseSignatureStatus,
    pub seen_nonces: Vec<String>,
    pub allowlist: HelperAllowlist,
}

impl HelperValidationContext {
    /// Creates a helper validation context.
    #[must_use]
    pub fn new(
        node_id: impl Into<String>,
        now_unix_seconds: u64,
        signature_status: LeaseSignatureStatus,
        seen_nonces: impl IntoIterator<Item = String>,
        allowlist: HelperAllowlist,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            now_unix_seconds,
            signature_status,
            seen_nonces: seen_nonces.into_iter().collect(),
            allowlist,
        }
    }
}

/// Fail-closed helper request rejection reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelperRejection {
    InvalidSignature,
    ExpiredLease,
    ReplayedNonce,
    NodeMismatch,
    LeaseMismatch,
    ActionMismatch,
    TargetMismatch,
    LocalAllowlistDenied,
}

/// Accepted helper invocation after lease, request, replay, and allowlist checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedHelperInvocation {
    pub lease_id: String,
    pub run_id: String,
    pub approval_id: String,
    pub action: ActionKind,
    pub target: ActionTarget,
}

/// Validates a helper request against its signed lease and local policy.
///
/// Cryptographic verification happens before this function and is represented
/// by [`LeaseSignatureStatus`]. The remaining checks are deterministic claim,
/// replay, target, and allowlist validation.
pub fn validate_helper_request(
    request: &HelperActionRequest,
    lease: &SignedCapabilityLease,
    context: &HelperValidationContext,
) -> Result<AcceptedHelperInvocation, HelperRejection> {
    let claims = &lease.claims;

    if context.signature_status != LeaseSignatureStatus::Valid {
        return Err(HelperRejection::InvalidSignature);
    }
    if claims.expires_at_unix_seconds <= context.now_unix_seconds {
        return Err(HelperRejection::ExpiredLease);
    }
    if context
        .seen_nonces
        .iter()
        .any(|nonce| nonce == &claims.nonce)
    {
        return Err(HelperRejection::ReplayedNonce);
    }
    if claims.node_id != context.node_id {
        return Err(HelperRejection::NodeMismatch);
    }
    if claims.lease_id != request.lease_id {
        return Err(HelperRejection::LeaseMismatch);
    }
    if claims.action != request.action {
        return Err(HelperRejection::ActionMismatch);
    }
    if claims.target != request.target {
        return Err(HelperRejection::TargetMismatch);
    }
    if !context
        .allowlist
        .permits(&claims.allowlist_entry_id, &request.action, &request.target)
    {
        return Err(HelperRejection::LocalAllowlistDenied);
    }

    Ok(AcceptedHelperInvocation {
        lease_id: claims.lease_id.clone(),
        run_id: claims.run_id.clone(),
        approval_id: claims.approval_id.clone(),
        action: claims.action.clone(),
        target: claims.target.clone(),
    })
}

/// A schedulable unit inside a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub layer: OperationalLayer,
    pub required_capabilities: Vec<Capability>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub lease_requests: Vec<ResourceLeaseRequest>,
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
            lease_requests: Vec::new(),
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

    /// Adds explicit lease requests.
    #[must_use]
    pub fn with_lease_requests(
        mut self,
        lease_requests: impl IntoIterator<Item = ResourceLeaseRequest>,
    ) -> Self {
        self.lease_requests = lease_requests.into_iter().collect();
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

/// Explicit dependency path from a lower-layer resource to an upper-layer resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDependencyPath {
    pub lower_resource_id: String,
    pub upper_resource_id: String,
}

impl ResourceDependencyPath {
    /// Creates a resource dependency path.
    #[must_use]
    pub fn new(lower_resource_id: impl Into<String>, upper_resource_id: impl Into<String>) -> Self {
        Self {
            lower_resource_id: lower_resource_id.into(),
            upper_resource_id: upper_resource_id.into(),
        }
    }
}

/// Why the scheduler allowed a task to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerRunReason {
    DependenciesSatisfiedNoConflicts,
}

/// Why the scheduler made a task wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerWaitReason {
    DependencyIncomplete {
        dependency_id: String,
    },
    ResourceConflict {
        resource_id: String,
        existing_lease_id: String,
        existing_mode: LeaseMode,
        requested_mode: LeaseMode,
    },
    LowerLayerDisruption {
        lower_resource_id: String,
        upper_resource_id: String,
        existing_lease_id: String,
        existing_mode: LeaseMode,
    },
}

/// Scheduler decision for a single task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerDecision {
    Run {
        task_id: String,
        reason: SchedulerRunReason,
    },
    Wait {
        task_id: String,
        reason: SchedulerWaitReason,
    },
}

/// In-memory scheduling snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerSnapshot {
    pub completed_task_ids: Vec<String>,
    pub active_leases: Vec<ResourceLease>,
    pub dependency_paths: Vec<ResourceDependencyPath>,
}

impl SchedulerSnapshot {
    /// Creates an empty scheduler snapshot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            completed_task_ids: Vec::new(),
            active_leases: Vec::new(),
            dependency_paths: Vec::new(),
        }
    }

    /// Adds completed task ids.
    #[must_use]
    pub fn with_completed_tasks(mut self, task_ids: impl IntoIterator<Item = String>) -> Self {
        self.completed_task_ids = task_ids.into_iter().collect();
        self
    }

    /// Adds active leases.
    #[must_use]
    pub fn with_active_leases(mut self, leases: impl IntoIterator<Item = ResourceLease>) -> Self {
        self.active_leases = leases.into_iter().collect();
        self
    }

    /// Adds lower-to-upper dependency paths.
    #[must_use]
    pub fn with_dependency_paths(
        mut self,
        paths: impl IntoIterator<Item = ResourceDependencyPath>,
    ) -> Self {
        self.dependency_paths = paths.into_iter().collect();
        self
    }

    /// Decides whether a task can run now.
    #[must_use]
    pub fn decide_task(&self, task: &Task) -> SchedulerDecision {
        if let Some(dependency_id) = task
            .dependencies
            .iter()
            .find(|dependency_id| !self.completed_task_ids.contains(dependency_id))
        {
            return SchedulerDecision::Wait {
                task_id: task.id.clone(),
                reason: SchedulerWaitReason::DependencyIncomplete {
                    dependency_id: dependency_id.clone(),
                },
            };
        }

        let requested_leases = task.effective_lease_requests();
        if let Some((request, existing)) = requested_leases.iter().find_map(|request| {
            self.active_leases
                .iter()
                .find(|existing| {
                    existing.resource_id == request.resource_id
                        && !existing.mode.is_compatible_with(request.mode)
                })
                .map(|existing| (request, existing))
        }) {
            return SchedulerDecision::Wait {
                task_id: task.id.clone(),
                reason: SchedulerWaitReason::ResourceConflict {
                    resource_id: request.resource_id.clone(),
                    existing_lease_id: existing.id.clone(),
                    existing_mode: existing.mode,
                    requested_mode: request.mode,
                },
            };
        }

        if let Some((path, lease)) = self.find_lower_layer_disruption(task) {
            return SchedulerDecision::Wait {
                task_id: task.id.clone(),
                reason: SchedulerWaitReason::LowerLayerDisruption {
                    lower_resource_id: path.lower_resource_id.clone(),
                    upper_resource_id: path.upper_resource_id.clone(),
                    existing_lease_id: lease.id.clone(),
                    existing_mode: lease.mode,
                },
            };
        }

        SchedulerDecision::Run {
            task_id: task.id.clone(),
            reason: SchedulerRunReason::DependenciesSatisfiedNoConflicts,
        }
    }

    fn find_lower_layer_disruption<'a>(
        &'a self,
        task: &Task,
    ) -> Option<(&'a ResourceDependencyPath, &'a ResourceLease)> {
        let requested_resources = task.mutated_resource_ids();
        self.active_leases
            .iter()
            .filter(|lease| matches!(lease.mode, LeaseMode::Drain | LeaseMode::Reboot))
            .find_map(|lease| {
                self.dependency_paths
                    .iter()
                    .find(|path| {
                        path.lower_resource_id == lease.resource_id
                            && requested_resources.contains(&path.upper_resource_id)
                            && task.layer != OperationalLayer::System
                    })
                    .map(|path| (path, lease))
            })
    }
}

impl Task {
    /// Returns explicit lease requests or default requests derived from reads and writes.
    #[must_use]
    pub fn effective_lease_requests(&self) -> Vec<ResourceLeaseRequest> {
        if !self.lease_requests.is_empty() {
            return self.lease_requests.clone();
        }

        let mut requests: Vec<ResourceLeaseRequest> = self
            .reads
            .iter()
            .map(|resource_id| {
                ResourceLeaseRequest::new(resource_id.clone(), LeaseMode::Observe, "task read")
            })
            .collect();
        requests.extend(self.writes.iter().map(|resource_id| {
            ResourceLeaseRequest::new(resource_id.clone(), LeaseMode::Exclusive, "task write")
        }));
        requests
    }

    fn mutated_resource_ids(&self) -> Vec<String> {
        let mut resource_ids = self.writes.clone();
        resource_ids.extend(
            self.lease_requests
                .iter()
                .filter(|request| request.mode.is_mutating())
                .map(|request| request.resource_id.clone()),
        );
        resource_ids
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
        ActionKind, ActionTarget, CapabilityLeaseClaims, HelperActionRequest, HelperAllowlist,
        HelperAllowlistEntry, HelperArgument, HelperRejection, HelperValidationContext,
        LeaseSignatureStatus, ResourceDependencyPath, SchedulerDecision, SchedulerSnapshot,
        SchedulerWaitReason, SignedCapabilityLease,
    };
    use super::{
        Capability, CapabilityFailure, CapabilityReport, ImpactSet, LeaseMode, OperatingSystem,
        OperationalLayer, Resource, ResourceKind, ResourceLease, ResourceScope, Run, RunState,
        SkippedVerification, Task, UnsupportedCapability, VerificationCheck, VerificationPlan,
        VerificationTier, is_valid_run_transition, validate_helper_request,
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

    #[test]
    fn scheduler_allows_independent_application_tasks_to_run_concurrently() {
        let snapshot = SchedulerSnapshot::empty().with_active_leases([ResourceLease::for_run(
            "lease-cart",
            "run-cart",
            "application:cart/service",
            LeaseMode::Exclusive,
            "cart restart",
        )]);
        let task = Task::new("restart-search", OperationalLayer::Application)
            .with_writes(["application:search/service".to_owned()]);

        assert_eq!(
            snapshot.decide_task(&task),
            SchedulerDecision::Run {
                task_id: "restart-search".to_owned(),
                reason: super::SchedulerRunReason::DependenciesSatisfiedNoConflicts,
            }
        );
    }

    #[test]
    fn scheduler_serializes_same_resource_mutations() {
        let snapshot = SchedulerSnapshot::empty().with_active_leases([ResourceLease::for_run(
            "lease-1",
            "run-1",
            "application:cart/service",
            LeaseMode::Exclusive,
            "cart restart",
        )]);
        let task = Task::new("restart-cart-again", OperationalLayer::Application)
            .with_writes(["application:cart/service".to_owned()]);

        assert_eq!(
            snapshot.decide_task(&task),
            SchedulerDecision::Wait {
                task_id: "restart-cart-again".to_owned(),
                reason: SchedulerWaitReason::ResourceConflict {
                    resource_id: "application:cart/service".to_owned(),
                    existing_lease_id: "lease-1".to_owned(),
                    existing_mode: LeaseMode::Exclusive,
                    requested_mode: LeaseMode::Exclusive,
                },
            }
        );
    }

    #[test]
    fn scheduler_blocks_upper_layer_mutation_during_system_drain() {
        let snapshot = SchedulerSnapshot::empty()
            .with_active_leases([ResourceLease::for_run(
                "lease-drain",
                "run-system",
                "system:node/prod-web-01/reboot",
                LeaseMode::Drain,
                "prepare reboot",
            )])
            .with_dependency_paths([ResourceDependencyPath::new(
                "system:node/prod-web-01/reboot",
                "application:on-node/prod-web-01",
            )]);
        let task = Task::new("restart-app", OperationalLayer::Application)
            .with_writes(["application:on-node/prod-web-01".to_owned()]);

        assert_eq!(
            snapshot.decide_task(&task),
            SchedulerDecision::Wait {
                task_id: "restart-app".to_owned(),
                reason: SchedulerWaitReason::LowerLayerDisruption {
                    lower_resource_id: "system:node/prod-web-01/reboot".to_owned(),
                    upper_resource_id: "application:on-node/prod-web-01".to_owned(),
                    existing_lease_id: "lease-drain".to_owned(),
                    existing_mode: LeaseMode::Drain,
                },
            }
        );
    }

    #[test]
    fn scheduler_waits_for_incomplete_task_dependency() {
        let snapshot =
            SchedulerSnapshot::empty().with_completed_tasks(["collect-status".to_owned()]);
        let task = Task::new("restart-service", OperationalLayer::System)
            .with_dependencies(["collect-status".to_owned(), "approval".to_owned()]);

        assert_eq!(
            snapshot.decide_task(&task),
            SchedulerDecision::Wait {
                task_id: "restart-service".to_owned(),
                reason: SchedulerWaitReason::DependencyIncomplete {
                    dependency_id: "approval".to_owned(),
                },
            }
        );
    }

    #[test]
    fn capability_report_supports_fail_closed_unsupported_shape() {
        let report = CapabilityReport::new(
            "openbsd-edge-01",
            OperatingSystem::OpenBsd,
            [
                Capability::new("os.openbsd"),
                Capability::new("service.openbsd-rcctl"),
            ],
            [UnsupportedCapability::new(
                "service.systemd",
                "OpenBSD uses rcctl, not systemd",
            )],
        );

        assert!(report.supports(&Capability::new("service.openbsd-rcctl")));
        assert!(!report.supports(&Capability::new("service.systemd")));

        let failure = CapabilityFailure::Unsupported(report.unsupported[0].clone());
        assert!(matches!(failure, CapabilityFailure::Unsupported(_)));
    }

    #[test]
    fn helper_validation_accepts_exact_signed_scoped_lease() {
        let target = ActionTarget::new("system:node/prod-web-01/service/sshd", "sshd");
        let lease = signed_service_restart_lease(target.clone(), 200);
        let request = HelperActionRequest::new(
            "lease-1",
            ActionKind::ServiceRestart,
            target,
            [HelperArgument::new("service", "sshd")],
        );
        let context = helper_context(100, LeaseSignatureStatus::Valid, []);

        let accepted = validate_helper_request(&request, &lease, &context)
            .expect("exact matching lease should validate");

        assert_eq!(accepted.lease_id, "lease-1");
        assert_eq!(accepted.run_id, "run-1");
        assert_eq!(accepted.approval_id, "approval-1");
        assert_eq!(accepted.action, ActionKind::ServiceRestart);
    }

    #[test]
    fn helper_validation_fails_closed_for_invalid_expired_or_replayed_leases() {
        let target = ActionTarget::new("system:node/prod-web-01/service/sshd", "sshd");
        let lease = signed_service_restart_lease(target.clone(), 200);
        let request = HelperActionRequest::new("lease-1", ActionKind::ServiceRestart, target, []);

        assert_eq!(
            validate_helper_request(
                &request,
                &lease,
                &helper_context(100, LeaseSignatureStatus::Invalid, [])
            ),
            Err(HelperRejection::InvalidSignature)
        );
        assert_eq!(
            validate_helper_request(
                &request,
                &lease,
                &helper_context(250, LeaseSignatureStatus::Valid, [])
            ),
            Err(HelperRejection::ExpiredLease)
        );
        assert_eq!(
            validate_helper_request(
                &request,
                &lease,
                &helper_context(100, LeaseSignatureStatus::Valid, ["nonce-1".to_owned()])
            ),
            Err(HelperRejection::ReplayedNonce)
        );
    }

    #[test]
    fn helper_validation_fails_closed_for_mismatched_action_target_and_allowlist() {
        let target = ActionTarget::new("system:node/prod-web-01/service/sshd", "sshd");
        let lease = signed_service_restart_lease(target.clone(), 200);

        let wrong_action =
            HelperActionRequest::new("lease-1", ActionKind::ServiceReload, target.clone(), []);
        assert_eq!(
            validate_helper_request(
                &wrong_action,
                &lease,
                &helper_context(100, LeaseSignatureStatus::Valid, [])
            ),
            Err(HelperRejection::ActionMismatch)
        );

        let wrong_target = HelperActionRequest::new(
            "lease-1",
            ActionKind::ServiceRestart,
            ActionTarget::new("system:node/prod-web-01/service/nginx", "nginx"),
            [],
        );
        assert_eq!(
            validate_helper_request(
                &wrong_target,
                &lease,
                &helper_context(100, LeaseSignatureStatus::Valid, [])
            ),
            Err(HelperRejection::TargetMismatch)
        );

        let denied_context = HelperValidationContext::new(
            "prod-web-01",
            100,
            LeaseSignatureStatus::Valid,
            [],
            HelperAllowlist::new([HelperAllowlistEntry::new(
                "allow-nginx-restart",
                ActionKind::ServiceRestart,
                "system:node/prod-web-01/service/nginx",
            )]),
        );
        let request = HelperActionRequest::new("lease-1", ActionKind::ServiceRestart, target, []);
        assert_eq!(
            validate_helper_request(&request, &lease, &denied_context),
            Err(HelperRejection::LocalAllowlistDenied)
        );
    }

    fn signed_service_restart_lease(
        target: ActionTarget,
        expires_at_unix_seconds: u64,
    ) -> SignedCapabilityLease {
        SignedCapabilityLease::new(
            CapabilityLeaseClaims::new(
                "lease-1",
                "run-1",
                "approval-1",
                "prod-web-01",
                ActionKind::ServiceRestart,
                target,
                "allow-sshd-restart",
                expires_at_unix_seconds,
                "nonce-1",
            ),
            "test-key",
            "test-signature",
        )
    }

    fn helper_context<const N: usize>(
        now_unix_seconds: u64,
        signature_status: LeaseSignatureStatus,
        seen_nonces: [String; N],
    ) -> HelperValidationContext {
        HelperValidationContext::new(
            "prod-web-01",
            now_unix_seconds,
            signature_status,
            seen_nonces,
            HelperAllowlist::new([HelperAllowlistEntry::new(
                "allow-sshd-restart",
                ActionKind::ServiceRestart,
                "system:node/prod-web-01/service/sshd",
            )]),
        )
    }
}
