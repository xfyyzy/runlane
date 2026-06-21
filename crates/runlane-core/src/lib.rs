//! Shared domain vocabulary for Runlane.
//!
//! This crate intentionally contains no network, database, or OS-specific code.

use std::collections::BTreeMap;

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

/// Fleet intent overlay tier. Later tiers override earlier tiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FleetOverlayTier {
    Global,
    Os(OperatingSystem),
    Layer(OperationalLayer),
    Role(String),
    Environment(String),
    Node(String),
}

impl FleetOverlayTier {
    /// Returns the overlay rank for global -> OS -> layer -> role -> environment -> node.
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Global => 0,
            Self::Os(_) => 1,
            Self::Layer(_) => 2,
            Self::Role(_) => 3,
            Self::Environment(_) => 4,
            Self::Node(_) => 5,
        }
    }
}

/// One desired-intent setting from the fleet repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetIntentSetting {
    pub key: String,
    pub value: String,
}

impl FleetIntentSetting {
    /// Creates a fleet intent setting.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// Overlay fragment loaded from one fleet-repo layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetOverlayFragment {
    pub tier: FleetOverlayTier,
    pub settings: Vec<FleetIntentSetting>,
}

impl FleetOverlayFragment {
    /// Creates a fleet overlay fragment.
    #[must_use]
    pub fn new(
        tier: FleetOverlayTier,
        settings: impl IntoIterator<Item = FleetIntentSetting>,
    ) -> Self {
        Self {
            tier,
            settings: settings.into_iter().collect(),
        }
    }
}

/// Resolved desired intent for one node or node group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFleetIntent {
    pub settings: Vec<FleetIntentSetting>,
}

impl ResolvedFleetIntent {
    /// Returns a resolved setting value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.settings
            .iter()
            .find(|setting| setting.key == key)
            .map(|setting| setting.value.as_str())
    }
}

/// Declares an operational layer in fleet intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetLayerDeclaration {
    pub layer: OperationalLayer,
    pub selector: String,
}

impl FleetLayerDeclaration {
    /// Creates a layer declaration for a fleet schema object.
    #[must_use]
    pub fn new(layer: OperationalLayer, selector: impl Into<String>) -> Self {
        Self {
            layer,
            selector: selector.into(),
        }
    }
}

/// Resolves fleet overlays using global -> OS -> layer -> role -> environment -> node order.
#[must_use]
pub fn resolve_fleet_overlays(
    fragments: impl IntoIterator<Item = FleetOverlayFragment>,
) -> ResolvedFleetIntent {
    let mut fragments: Vec<FleetOverlayFragment> = fragments.into_iter().collect();
    fragments.sort_by_key(|fragment| fragment.tier.rank());

    let mut resolved = BTreeMap::new();
    for fragment in fragments {
        for setting in fragment.settings {
            resolved.insert(setting.key, setting.value);
        }
    }

    ResolvedFleetIntent {
        settings: resolved
            .into_iter()
            .map(|(key, value)| FleetIntentSetting { key, value })
            .collect(),
    }
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

    /// Returns true when every skipped verification has an audit-ready reason.
    #[must_use]
    pub fn skipped_checks_have_reasons(&self) -> bool {
        self.skipped
            .iter()
            .all(|skipped| !skipped.reason.trim().is_empty())
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

/// Enrolled agent identity used by the pull protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentity {
    pub node_id: String,
    pub certificate_fingerprint: String,
}

impl AgentIdentity {
    /// Creates an agent identity.
    #[must_use]
    pub fn new(node_id: impl Into<String>, certificate_fingerprint: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            certificate_fingerprint: certificate_fingerprint.into(),
        }
    }
}

/// Task envelope pulled by an agent from the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTaskEnvelope {
    pub envelope_id: String,
    pub run_id: String,
    pub task_id: String,
    pub node_id: String,
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub nonce: String,
    pub required_capabilities: Vec<Capability>,
    pub audit_correlation_id: String,
}

impl AgentTaskEnvelope {
    /// Creates an agent task envelope.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope_id: impl Into<String>,
        run_id: impl Into<String>,
        task_id: impl Into<String>,
        node_id: impl Into<String>,
        issued_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
        nonce: impl Into<String>,
        required_capabilities: impl IntoIterator<Item = Capability>,
        audit_correlation_id: impl Into<String>,
    ) -> Self {
        Self {
            envelope_id: envelope_id.into(),
            run_id: run_id.into(),
            task_id: task_id.into(),
            node_id: node_id.into(),
            issued_at_unix_seconds,
            expires_at_unix_seconds,
            nonce: nonce.into(),
            required_capabilities: required_capabilities.into_iter().collect(),
            audit_correlation_id: audit_correlation_id.into(),
        }
    }
}

/// Agent protocol validation context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProtocolContext {
    pub node_id: String,
    pub now_unix_seconds: u64,
    pub seen_nonces: Vec<String>,
}

impl AgentProtocolContext {
    /// Creates agent protocol validation context.
    #[must_use]
    pub fn new(
        node_id: impl Into<String>,
        now_unix_seconds: u64,
        seen_nonces: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            now_unix_seconds,
            seen_nonces: seen_nonces.into_iter().collect(),
        }
    }
}

/// Accepted pulled task after node, expiry, and replay checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedAgentTask {
    pub envelope_id: String,
    pub run_id: String,
    pub task_id: String,
    pub audit_correlation_id: String,
}

/// Fail-closed task envelope rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentTaskRejection {
    NodeMismatch,
    ExpiredEnvelope,
    ReplayedNonce,
}

/// Validates a pulled task envelope before local execution.
pub fn validate_agent_task_envelope(
    envelope: &AgentTaskEnvelope,
    context: &AgentProtocolContext,
) -> Result<AcceptedAgentTask, AgentTaskRejection> {
    if envelope.node_id != context.node_id {
        return Err(AgentTaskRejection::NodeMismatch);
    }
    if envelope.expires_at_unix_seconds <= context.now_unix_seconds {
        return Err(AgentTaskRejection::ExpiredEnvelope);
    }
    if context
        .seen_nonces
        .iter()
        .any(|nonce| nonce == &envelope.nonce)
    {
        return Err(AgentTaskRejection::ReplayedNonce);
    }

    Ok(AcceptedAgentTask {
        envelope_id: envelope.envelope_id.clone(),
        run_id: envelope.run_id.clone(),
        task_id: envelope.task_id.clone(),
        audit_correlation_id: envelope.audit_correlation_id.clone(),
    })
}

/// Result status reported by an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentResultStatus {
    Succeeded,
    Failed,
}

/// Agent result submission shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResultSubmission {
    pub envelope_id: String,
    pub run_id: String,
    pub task_id: String,
    pub node_id: String,
    pub nonce: String,
    pub status: AgentResultStatus,
    pub evidence: Vec<EvidenceEnvelope>,
    pub audit_correlation_id: String,
}

impl AgentResultSubmission {
    /// Creates an agent result submission.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope_id: impl Into<String>,
        run_id: impl Into<String>,
        task_id: impl Into<String>,
        node_id: impl Into<String>,
        nonce: impl Into<String>,
        status: AgentResultStatus,
        evidence: impl IntoIterator<Item = EvidenceEnvelope>,
        audit_correlation_id: impl Into<String>,
    ) -> Self {
        Self {
            envelope_id: envelope_id.into(),
            run_id: run_id.into(),
            task_id: task_id.into(),
            node_id: node_id.into(),
            nonce: nonce.into(),
            status,
            evidence: evidence.into_iter().collect(),
            audit_correlation_id: audit_correlation_id.into(),
        }
    }
}

/// Why an agent result was spooled locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpoolReason {
    ServerUnavailable,
    SubmissionRejected(String),
}

/// Local spool item for failed result submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSpoolItem {
    pub spool_id: String,
    pub reason: SpoolReason,
    pub result: AgentResultSubmission,
}

impl LocalSpoolItem {
    /// Creates a local spool item.
    #[must_use]
    pub fn new(
        spool_id: impl Into<String>,
        reason: SpoolReason,
        result: AgentResultSubmission,
    ) -> Self {
        Self {
            spool_id: spool_id.into(),
            reason,
            result,
        }
    }
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

/// Policy toggles used by the impact-scoped verification planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationPolicy {
    pub require_dependent_app_canary_for_platform: bool,
    pub require_broad_package_audit_for_system_package_mutation: bool,
}

impl VerificationPolicy {
    /// Creates a conservative default v0.1 verification policy.
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            require_dependent_app_canary_for_platform: true,
            require_broad_package_audit_for_system_package_mutation: true,
        }
    }

    /// Creates a minimal policy useful for low-risk tests and demos.
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            require_dependent_app_canary_for_platform: false,
            require_broad_package_audit_for_system_package_mutation: false,
        }
    }
}

/// Selects verification checks from action, operational layer, impact, and policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationPlanner {
    pub policy: VerificationPolicy,
}

impl VerificationPlanner {
    /// Creates a planner with a policy.
    #[must_use]
    pub const fn new(policy: VerificationPolicy) -> Self {
        Self { policy }
    }

    /// Plans verification for a typed action and impact set.
    #[must_use]
    pub fn plan(&self, action: &ActionKind, impact: &ImpactSet) -> VerificationPlan {
        match (impact.layer, action) {
            (
                OperationalLayer::Application,
                ActionKind::ServiceRestart | ActionKind::ServiceReload,
            ) => self.plan_application_service_change(action, impact),
            (OperationalLayer::Platform, ActionKind::ServiceReload) => {
                self.plan_platform_reload(impact)
            }
            (OperationalLayer::System, _)
                if impact
                    .writes
                    .iter()
                    .any(|resource| resource.contains("package-db")) =>
            {
                self.plan_system_package_mutation(impact)
            }
            (_, ActionKind::ServiceRestart | ActionKind::ServiceReload) => {
                self.plan_generic_service_change(action, impact)
            }
            _ => self.plan_generic_typed_action(impact),
        }
    }

    fn plan_application_service_change(
        &self,
        action: &ActionKind,
        impact: &ImpactSet,
    ) -> VerificationPlan {
        let mut plan = self.plan_generic_service_change(action, impact);
        plan.skipped.extend([
            SkippedVerification::new(
                "package_audit",
                "application service change did not mutate system package database",
            ),
            SkippedVerification::new(
                "full_disk_scan",
                "application service change did not mutate filesystem allocation",
            ),
            SkippedVerification::new(
                "firewall_audit",
                "application service change did not mutate firewall rules",
            ),
        ]);
        plan
    }

    fn plan_platform_reload(&self, impact: &ImpactSet) -> VerificationPlan {
        let mut required = vec![VerificationCheck::new(
            "config_syntax_valid",
            primary_resource(impact),
            VerificationTier::Precondition,
        )];
        required.push(VerificationCheck::new(
            "platform_accepts_connections",
            primary_resource(impact),
            VerificationTier::DirectImpact,
        ));
        if self.policy.require_dependent_app_canary_for_platform {
            required.push(VerificationCheck::new(
                "dependent_app_canary",
                dependent_resource(impact),
                VerificationTier::Dependent,
            ));
        }

        VerificationPlan {
            required,
            conditional: Vec::new(),
            skipped: vec![SkippedVerification::new(
                "full_package_audit",
                "platform config reload did not mutate system package database",
            )],
        }
    }

    fn plan_system_package_mutation(&self, impact: &ImpactSet) -> VerificationPlan {
        let mut required = vec![
            VerificationCheck::new(
                "package_db_consistent",
                primary_resource(impact),
                VerificationTier::DirectImpact,
            ),
            VerificationCheck::new(
                "changed_files_classified",
                primary_resource(impact),
                VerificationTier::DirectImpact,
            ),
            VerificationCheck::new(
                "affected_services_identified",
                primary_resource(impact),
                VerificationTier::Dependent,
            ),
        ];
        if self
            .policy
            .require_broad_package_audit_for_system_package_mutation
        {
            required.push(VerificationCheck::new(
                "package_audit",
                primary_resource(impact),
                VerificationTier::BroadRegression,
            ));
        }

        VerificationPlan {
            required,
            conditional: vec![VerificationCheck::new(
                "service_health_for_affected_services",
                dependent_resource(impact),
                VerificationTier::Dependent,
            )],
            skipped: vec![SkippedVerification::new(
                "full_disk_scan",
                "package mutation classified changed files without broad filesystem writes",
            )],
        }
    }

    fn plan_generic_service_change(
        &self,
        action: &ActionKind,
        impact: &ImpactSet,
    ) -> VerificationPlan {
        let direct_check = match action {
            ActionKind::ServiceReload => "service_reloaded",
            _ => "service_active",
        };
        VerificationPlan::required([
            VerificationCheck::new(
                "helper_result_matches_request",
                primary_resource(impact),
                VerificationTier::DirectImpact,
            ),
            VerificationCheck::new(
                direct_check,
                primary_resource(impact),
                VerificationTier::DirectImpact,
            ),
        ])
    }

    fn plan_generic_typed_action(&self, impact: &ImpactSet) -> VerificationPlan {
        VerificationPlan::required([VerificationCheck::new(
            "helper_result_matches_request",
            primary_resource(impact),
            VerificationTier::DirectImpact,
        )])
    }
}

fn primary_resource(impact: &ImpactSet) -> String {
    impact
        .writes
        .first()
        .cloned()
        .unwrap_or_else(|| format!("{:?}:unknown", impact.layer))
}

fn dependent_resource(impact: &ImpactSet) -> String {
    impact
        .may_affect
        .first()
        .cloned()
        .unwrap_or_else(|| primary_resource(impact))
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

/// Approval outcome recorded in the audit ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalOutcome {
    Approved,
    Rejected,
}

/// Append-only audit event payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEventKind {
    RunStateTransition {
        from: RunState,
        to: RunState,
    },
    ResourceLeaseRequested {
        task_id: String,
        resource_id: String,
        mode: LeaseMode,
    },
    ResourceLeaseGranted {
        lease_id: String,
        resource_id: String,
        mode: LeaseMode,
    },
    ResourceLeaseDenied {
        resource_id: String,
        mode: LeaseMode,
        reason: String,
    },
    SchedulerDecision {
        decision: SchedulerDecision,
    },
    VerificationSelected {
        plan: VerificationPlan,
    },
    VerificationSkipped {
        skipped: SkippedVerification,
    },
    ApprovalDecision {
        approval_id: String,
        actor: String,
        outcome: ApprovalOutcome,
    },
    ActionResult {
        action: ActionKind,
        target: ActionTarget,
        status: HelperActionStatus,
    },
    CognitiveReceiptGenerated {
        receipt_id: String,
    },
}

/// One immutable audit event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    pub id: String,
    pub run_id: String,
    pub sequence: u64,
    pub kind: AuditEventKind,
}

impl AuditEvent {
    /// Creates an audit event.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        run_id: impl Into<String>,
        sequence: u64,
        kind: AuditEventKind,
    ) -> Self {
        Self {
            id: id.into(),
            run_id: run_id.into(),
            sequence,
            kind,
        }
    }
}

/// Append-only in-memory audit ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLedger {
    events: Vec<AuditEvent>,
}

impl AuditLedger {
    /// Creates an empty ledger.
    #[must_use]
    pub const fn empty() -> Self {
        Self { events: Vec::new() }
    }

    /// Appends an event when its sequence is the next monotonic sequence.
    pub fn append(&mut self, event: AuditEvent) -> Result<(), AuditAppendError> {
        let expected = self.next_sequence();
        if event.sequence != expected {
            return Err(AuditAppendError::NonMonotonicSequence {
                expected,
                actual: event.sequence,
            });
        }
        self.events.push(event);
        Ok(())
    }

    /// Returns the immutable event slice.
    #[must_use]
    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }

    /// Returns the next expected sequence number.
    #[must_use]
    pub fn next_sequence(&self) -> u64 {
        self.events.len() as u64 + 1
    }
}

/// Audit append failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAppendError {
    NonMonotonicSequence { expected: u64, actual: u64 },
}

/// Final run receipt intended for operator handoff and later review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CognitiveReceipt {
    pub id: String,
    pub run_id: String,
    pub layer: OperationalLayer,
    pub impact: ImpactSet,
    pub evidence: Vec<EvidenceEnvelope>,
    pub verification: VerificationPlan,
    pub skipped_checks: Vec<SkippedVerification>,
    pub residual_risk: String,
    pub takeover_notes: String,
    pub rollback_notes: String,
}

impl CognitiveReceipt {
    /// Creates a cognitive receipt.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        run_id: impl Into<String>,
        layer: OperationalLayer,
        impact: ImpactSet,
        evidence: impl IntoIterator<Item = EvidenceEnvelope>,
        verification: VerificationPlan,
        residual_risk: impl Into<String>,
        takeover_notes: impl Into<String>,
        rollback_notes: impl Into<String>,
    ) -> Self {
        let skipped_checks = verification.skipped.clone();
        Self {
            id: id.into(),
            run_id: run_id.into(),
            layer,
            impact,
            evidence: evidence.into_iter().collect(),
            verification,
            skipped_checks,
            residual_risk: residual_risk.into(),
            takeover_notes: takeover_notes.into(),
            rollback_notes: rollback_notes.into(),
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
    use super::ApprovalOutcome;
    use super::{
        ActionKind, ActionTarget, CapabilityLeaseClaims, HelperActionRequest, HelperAllowlist,
        HelperAllowlistEntry, HelperArgument, HelperRejection, HelperValidationContext,
        LeaseSignatureStatus, ResourceDependencyPath, SchedulerDecision, SchedulerSnapshot,
        SchedulerWaitReason, SignedCapabilityLease,
    };
    use super::{
        AgentProtocolContext, AgentResultStatus, AgentResultSubmission, AgentTaskEnvelope,
        AgentTaskRejection, AuditAppendError, AuditEvent, AuditEventKind, AuditLedger, Capability,
        CapabilityFailure, CapabilityReport, CognitiveReceipt, EvidenceEnvelope,
        FleetIntentSetting, FleetLayerDeclaration, FleetOverlayFragment, FleetOverlayTier,
        ImpactSet, LeaseMode, LocalSpoolItem, OperatingSystem, OperationalLayer,
        ResolvedFleetIntent, Resource, ResourceKind, ResourceLease, ResourceScope, Run, RunState,
        SkippedVerification, SpoolReason, Task, UnsupportedCapability, VerificationCheck,
        VerificationPlan, VerificationPlanner, VerificationPolicy, VerificationTier,
        is_valid_run_transition, resolve_fleet_overlays, validate_agent_task_envelope,
        validate_helper_request,
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
    fn fleet_overlay_resolution_uses_declared_order() {
        let resolved = resolve_fleet_overlays([
            FleetOverlayFragment::new(
                FleetOverlayTier::Node("prod-web-01".to_owned()),
                [
                    FleetIntentSetting::new("policy.profile", "node-production"),
                    FleetIntentSetting::new("runbook.service-unhealthy.approval", "required"),
                ],
            ),
            FleetOverlayFragment::new(
                FleetOverlayTier::Global,
                [
                    FleetIntentSetting::new("policy.profile", "baseline"),
                    FleetIntentSetting::new("verification.tier3.default", "false"),
                ],
            ),
            FleetOverlayFragment::new(
                FleetOverlayTier::Os(OperatingSystem::Linux),
                [FleetIntentSetting::new("service.driver", "systemd")],
            ),
            FleetOverlayFragment::new(
                FleetOverlayTier::Layer(OperationalLayer::System),
                [FleetIntentSetting::new(
                    "verification.tier3.default",
                    "true",
                )],
            ),
            FleetOverlayFragment::new(
                FleetOverlayTier::Role("web".to_owned()),
                [FleetIntentSetting::new(
                    "runbook.service-unhealthy.approval",
                    "conditional",
                )],
            ),
            FleetOverlayFragment::new(
                FleetOverlayTier::Environment("prod".to_owned()),
                [FleetIntentSetting::new("policy.profile", "production")],
            ),
        ]);

        assert_eq!(resolved.get("service.driver"), Some("systemd"));
        assert_eq!(resolved.get("verification.tier3.default"), Some("true"));
        assert_eq!(resolved.get("policy.profile"), Some("node-production"));
        assert_eq!(
            resolved.get("runbook.service-unhealthy.approval"),
            Some("required")
        );
    }

    #[test]
    fn fleet_schema_can_declare_all_operational_layers() {
        let declarations = [
            FleetLayerDeclaration::new(OperationalLayer::System, "labels.runlane.io/layer=system"),
            FleetLayerDeclaration::new(
                OperationalLayer::Platform,
                "labels.runlane.io/layer=platform",
            ),
            FleetLayerDeclaration::new(
                OperationalLayer::Application,
                "labels.runlane.io/layer=application",
            ),
        ];

        let resolved = ResolvedFleetIntent {
            settings: declarations
                .iter()
                .map(|declaration| {
                    FleetIntentSetting::new(
                        format!("layer.{:?}.selector", declaration.layer),
                        declaration.selector.clone(),
                    )
                })
                .collect(),
        };

        assert!(resolved.get("layer.System.selector").is_some());
        assert!(resolved.get("layer.Platform.selector").is_some());
        assert!(resolved.get("layer.Application.selector").is_some());
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
    fn verification_planner_keeps_application_restart_narrow() {
        let impact = ImpactSet::writes(
            OperationalLayer::Application,
            ["application:blog/service".to_owned()],
        )
        .with_does_not_affect([
            "system:node/prod-web-01/package-db".to_owned(),
            "system:node/prod-web-01/firewall".to_owned(),
        ]);

        let plan = VerificationPlanner::new(VerificationPolicy::conservative())
            .plan(&ActionKind::ServiceRestart, &impact);

        assert!(
            plan.required
                .iter()
                .any(|check| check.id == "service_active")
        );
        assert!(
            plan.skipped
                .iter()
                .any(|skipped| skipped.check_id == "package_audit")
        );
        assert!(
            plan.skipped
                .iter()
                .any(|skipped| skipped.check_id == "full_disk_scan")
        );
        assert!(plan.skipped_checks_have_reasons());
    }

    #[test]
    fn verification_planner_can_select_broader_system_package_checks() {
        let impact = ImpactSet::writes(
            OperationalLayer::System,
            ["system:node/prod-web-01/package-db".to_owned()],
        )
        .with_may_affect(["application:on-node/prod-web-01".to_owned()]);

        let plan = VerificationPlanner::new(VerificationPolicy::conservative())
            .plan(&ActionKind::RunAllowlistedScript, &impact);

        assert!(
            plan.required.iter().any(|check| check.id == "package_audit"
                && check.tier == VerificationTier::BroadRegression)
        );
        assert!(
            plan.required
                .iter()
                .any(|check| check.id == "affected_services_identified")
        );
        assert!(plan.skipped_checks_have_reasons());
    }

    #[test]
    fn verification_planner_selects_platform_dependent_canary_when_policy_requires() {
        let impact = ImpactSet::writes(
            OperationalLayer::Platform,
            ["platform:gateway/nginx-main/config".to_owned()],
        )
        .with_may_affect(["application:depends-on/gateway/nginx-main".to_owned()]);

        let plan = VerificationPlanner::new(VerificationPolicy::conservative())
            .plan(&ActionKind::ServiceReload, &impact);

        assert!(
            plan.required
                .iter()
                .any(|check| check.id == "dependent_app_canary"
                    && check.tier == VerificationTier::Dependent)
        );
        assert!(plan.skipped_checks_have_reasons());
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
    fn agent_task_envelope_validates_node_expiry_and_replay() {
        let envelope = AgentTaskEnvelope::new(
            "env-1",
            "run-1",
            "task-1",
            "prod-web-01",
            100,
            200,
            "nonce-1",
            [Capability::new("service.systemd")],
            "audit-1",
        );

        let accepted = validate_agent_task_envelope(
            &envelope,
            &AgentProtocolContext::new("prod-web-01", 150, []),
        )
        .expect("fresh envelope for matching node should validate");
        assert_eq!(accepted.audit_correlation_id, "audit-1");

        assert_eq!(
            validate_agent_task_envelope(
                &envelope,
                &AgentProtocolContext::new("other-node", 150, [])
            ),
            Err(AgentTaskRejection::NodeMismatch)
        );
        assert_eq!(
            validate_agent_task_envelope(
                &envelope,
                &AgentProtocolContext::new("prod-web-01", 250, [])
            ),
            Err(AgentTaskRejection::ExpiredEnvelope)
        );
        assert_eq!(
            validate_agent_task_envelope(
                &envelope,
                &AgentProtocolContext::new("prod-web-01", 150, ["nonce-1".to_owned()])
            ),
            Err(AgentTaskRejection::ReplayedNonce)
        );
    }

    #[test]
    fn failed_result_submission_can_be_spooled_with_audit_metadata() {
        let result = AgentResultSubmission::new(
            "env-1",
            "run-1",
            "task-1",
            "prod-web-01",
            "result-nonce-1",
            AgentResultStatus::Succeeded,
            [EvidenceEnvelope::text("service_status", "sshd active")],
            "audit-1",
        );
        let spool_item = LocalSpoolItem::new("spool-1", SpoolReason::ServerUnavailable, result);

        assert_eq!(spool_item.result.run_id, "run-1");
        assert_eq!(spool_item.result.task_id, "task-1");
        assert_eq!(spool_item.result.node_id, "prod-web-01");
        assert_eq!(spool_item.result.audit_correlation_id, "audit-1");
        assert_eq!(spool_item.result.evidence.len(), 1);
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

    #[test]
    fn audit_ledger_is_append_only_with_monotonic_sequences() {
        let mut ledger = AuditLedger::empty();
        ledger
            .append(AuditEvent::new(
                "event-1",
                "run-1",
                1,
                AuditEventKind::RunStateTransition {
                    from: RunState::Created,
                    to: RunState::Planned,
                },
            ))
            .expect("first event should append");

        let err = ledger
            .append(AuditEvent::new(
                "event-3",
                "run-1",
                3,
                AuditEventKind::CognitiveReceiptGenerated {
                    receipt_id: "receipt-1".to_owned(),
                },
            ))
            .expect_err("ledger rejects skipped sequence numbers");

        assert_eq!(
            err,
            AuditAppendError::NonMonotonicSequence {
                expected: 2,
                actual: 3,
            }
        );
        assert_eq!(ledger.events().len(), 1);
        assert_eq!(ledger.next_sequence(), 2);
    }

    #[test]
    fn audit_events_cover_happy_path_sequence_and_receipt() {
        let service_id = "system:node/prod-web-01/service/sshd".to_owned();
        let target = ActionTarget::new(service_id.clone(), "sshd");
        let impact = ImpactSet::writes(OperationalLayer::System, [service_id.clone()])
            .with_may_affect(["application:on-node/prod-web-01".to_owned()]);
        let verification = VerificationPlanner::new(VerificationPolicy::conservative())
            .plan(&ActionKind::ServiceRestart, &impact);
        let receipt = CognitiveReceipt::new(
            "receipt-1",
            "run-1",
            OperationalLayer::System,
            impact,
            [EvidenceEnvelope::text("service_status", "sshd failed")],
            verification.clone(),
            "service may fail again if root cause remains",
            "operator can inspect sshd logs and service status",
            "restart previous config or stop the run manually",
        );

        let mut ledger = AuditLedger::empty();
        append_events(
            &mut ledger,
            [
                AuditEventKind::RunStateTransition {
                    from: RunState::Created,
                    to: RunState::Planned,
                },
                AuditEventKind::SchedulerDecision {
                    decision: SchedulerDecision::Run {
                        task_id: "collect-status".to_owned(),
                        reason: super::SchedulerRunReason::DependenciesSatisfiedNoConflicts,
                    },
                },
                AuditEventKind::ResourceLeaseRequested {
                    task_id: "restart-service".to_owned(),
                    resource_id: service_id.clone(),
                    mode: LeaseMode::Exclusive,
                },
                AuditEventKind::ResourceLeaseGranted {
                    lease_id: "lease-1".to_owned(),
                    resource_id: service_id,
                    mode: LeaseMode::Exclusive,
                },
                AuditEventKind::ApprovalDecision {
                    approval_id: "approval-1".to_owned(),
                    actor: "operator".to_owned(),
                    outcome: ApprovalOutcome::Approved,
                },
                AuditEventKind::ActionResult {
                    action: ActionKind::ServiceRestart,
                    target,
                    status: super::HelperActionStatus::Succeeded,
                },
                AuditEventKind::VerificationSelected { plan: verification },
                AuditEventKind::CognitiveReceiptGenerated {
                    receipt_id: receipt.id.clone(),
                },
            ],
        );

        assert_eq!(ledger.events().len(), 8);
        assert_eq!(receipt.layer, OperationalLayer::System);
        assert_eq!(receipt.evidence.len(), 1);
        assert_eq!(receipt.skipped_checks, receipt.verification.skipped);
        assert!(receipt.residual_risk.contains("root cause"));
        assert!(receipt.takeover_notes.contains("operator"));
        assert!(receipt.rollback_notes.contains("restart previous config"));
    }

    fn append_events<const N: usize>(ledger: &mut AuditLedger, kinds: [AuditEventKind; N]) {
        for (index, kind) in kinds.into_iter().enumerate() {
            let sequence = u64::try_from(index).expect("test event index fits in u64") + 1;
            ledger
                .append(AuditEvent::new(
                    format!("event-{sequence}"),
                    "run-1",
                    sequence,
                    kind,
                ))
                .expect("test events have monotonic sequences");
        }
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
