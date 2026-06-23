mod config;
mod platform;

use std::{env, error::Error, fmt, path::PathBuf};

use config::{
    AgentConfig, AgentConfigError, InitConfigOptions, InstallIdentityOptions,
    format_operating_system, init_config, install_identity, parse_operating_system, show_config,
    validate_agent_state,
};
use platform::{
    CollectorExecutionError, CollectorRequest, EvidenceKind, NativeBackend, PlatformBackend,
};
use runlane_core::{
    ActionKind, ActionTarget, AgentTaskEnvelope, AuditEvent, AuditEventKind, AuditLedger,
    Capability, CapabilityLeaseClaims, EvidenceEnvelope, HelperActionRequest, HelperActionStatus,
    HelperAllowlist, HelperAllowlistEntry, HelperArgument, HelperValidationContext, LeaseMode,
    LeaseSignatureStatus, ServiceUnhealthyRunbookPlan, ServiceUnhealthyRunbookRequest,
    SignedCapabilityLease, Task, VerificationPlanner, VerificationPolicy,
    analyzer::{
        ProposalPolicy, ProposedActionKind, StructuredProposal, analyze_service_unhealthy,
        validate_proposal,
    },
    approval::{ApprovalRecord, ApprovalStore},
    durable::LocalServerState,
    receipt::generate_operator_receipt,
    runtime::{
        AgentEnrollmentRequest, ControlPlane, EnrollmentToken, PendingAgentTask, TypedTaskPayload,
    },
    validate_helper_request,
};

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = run_cli(&args) {
        eprintln!("runlane-agent: {error}");
        std::process::exit(1);
    }
}

fn run_cli(args: &[String]) -> Result<(), CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        print_platform_probe();
        return Ok(());
    };

    match command {
        "demo-enroll-pull" if args.len() == 1 => {
            demo_enroll_pull();
            Ok(())
        }
        "config" => run_config_command(&args[1..]),
        "identity" => run_identity_command(&args[1..]),
        "collect-smoke" => run_collect_smoke(&args[1..]),
        "dogfood-service-unhealthy" => run_dogfood_service_unhealthy(&args[1..]),
        "run" => run_agent(&args[1..]),
        _ => Err(CliError::usage()),
    }
}

fn print_platform_probe() {
    let backend = NativeBackend::current();
    let report = backend.capability_report("local-node");
    let fixture_stub_count = backend.parser_fixture_stubs().len();
    let collector_count = backend.collector_specs().len();
    let service_fixture_probe_ok = backend
        .collect_fixture(
            EvidenceKind::ServiceStatus,
            service_status_fixture(backend.os()),
        )
        .is_ok();
    let capability_probe_ok = report
        .capabilities
        .first()
        .is_some_and(|capability| backend.require_capability(capability).is_ok());
    println!(
        "runlane-agent skeleton; detected_os={:?}; capabilities={}; unsupported={}; fixture_stubs={}; collectors={}; capability_probe_ok={}; service_fixture_probe_ok={}",
        report.os,
        report.capabilities.len(),
        report.unsupported.len(),
        fixture_stub_count,
        collector_count,
        capability_probe_ok,
        service_fixture_probe_ok
    );
}

fn run_config_command(args: &[String]) -> Result<(), CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(CliError::usage());
    };
    match command {
        "init" => {
            let mut flags = FlagParser::new(&args[1..]);
            let config_path = flags.required_path("--config")?;
            let node_id = flags.required("--node-id")?;
            let server_url = flags.required("--server-url")?;
            let server_trust_root_path = flags.required_path("--trust-root-path")?;
            let identity_path = flags.required_path("--identity-path")?;
            let certificate_path = flags.required_path("--certificate-path")?;
            let private_key_path = flags.required_path("--private-key-path")?;
            let spool_dir = flags.required_path("--spool-dir")?;
            let platform_family = flags
                .optional("--platform-family")?
                .map_or_else(current_supported_platform, |value| {
                    parse_operating_system(&value).map_err(CliError::from)
                })?;
            let force = flags.present("--force");
            flags.finish()?;

            let config = AgentConfig::new(
                node_id,
                server_url,
                server_trust_root_path,
                identity_path,
                certificate_path,
                private_key_path,
                spool_dir,
                platform_family,
            );
            init_config(&InitConfigOptions {
                config_path: config_path.clone(),
                config,
                force,
            })?;
            println!(
                "runlane-agent config init ok; config={}",
                config_path.display()
            );
            Ok(())
        }
        "show" => {
            let mut flags = FlagParser::new(&args[1..]);
            let config_path = flags.required_path("--config")?;
            flags.finish()?;
            print!("{}", show_config(&config_path)?);
            Ok(())
        }
        "validate" => {
            let mut flags = FlagParser::new(&args[1..]);
            let config_path = flags.required_path("--config")?;
            flags.finish()?;
            let state = validate_agent_state(&config_path, NativeBackend::current().os())?;
            println!(
                "runlane-agent config validate ok; node={}; platform={}; identity=present; spool={}",
                state.config.node_id,
                format_operating_system(state.config.platform_family),
                state.config.spool_dir.display()
            );
            Ok(())
        }
        _ => Err(CliError::usage()),
    }
}

fn run_identity_command(args: &[String]) -> Result<(), CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(CliError::usage());
    };
    match command {
        "install" => {
            let mut flags = FlagParser::new(&args[1..]);
            let config_path = flags.required_path("--config")?;
            let certificate_fingerprint = flags.required("--certificate-fingerprint")?;
            let enrolled_at_unix_seconds = flags.required_u64("--enrolled-at")?;
            let expires_at_unix_seconds = flags.optional_u64("--expires-at")?;
            let force = flags.present("--force");
            flags.finish()?;
            let identity = install_identity(&InstallIdentityOptions {
                config_path,
                certificate_fingerprint,
                enrolled_at_unix_seconds,
                expires_at_unix_seconds,
                force,
            })?;
            println!(
                "runlane-agent identity install ok; node={}; platform={}; certificate_fingerprint={}",
                identity.node_id,
                format_operating_system(identity.platform_family),
                identity.certificate_fingerprint
            );
            Ok(())
        }
        _ => Err(CliError::usage()),
    }
}

fn run_agent(args: &[String]) -> Result<(), CliError> {
    let mut flags = FlagParser::new(args);
    let config_path = flags.required_path("--config")?;
    flags.finish()?;
    let state = validate_agent_state(&config_path, NativeBackend::current().os())?;
    println!(
        "runlane-agent run; node={}; server_url={}; platform={}; identity_fingerprint={}; spool={}",
        state.config.node_id,
        state.config.server_url,
        format_operating_system(state.config.platform_family),
        state.identity.certificate_fingerprint,
        state.config.spool_dir.display()
    );
    Ok(())
}

fn run_collect_smoke(args: &[String]) -> Result<(), CliError> {
    let mut flags = FlagParser::new(args);
    let service_name = flags.required("--service")?;
    flags.finish()?;

    let backend = NativeBackend::current();
    if matches!(backend.os(), runlane_core::OperatingSystem::Unknown) {
        return Err(CliError::Config(AgentConfigError::InvalidField {
            field: "platform_family",
            reason: "native collector smoke requires Linux, FreeBSD, or OpenBSD".to_owned(),
        }));
    }

    for kind in service_unhealthy_collector_kinds() {
        let request = service_unhealthy_collector_request(kind, &service_name)?;
        let evidence = backend.collect_native(&request)?;
        println!(
            "runlane-agent collect-smoke; os={:?}; kind={kind:?}; source={}; bytes={}",
            backend.os(),
            evidence.source,
            evidence.body.len()
        );
    }
    Ok(())
}

fn service_unhealthy_collector_kinds() -> [EvidenceKind; 5] {
    [
        EvidenceKind::ServiceStatus,
        EvidenceKind::RecentLogs,
        EvidenceKind::Disk,
        EvidenceKind::Process,
        EvidenceKind::Socket,
    ]
}

fn service_unhealthy_collector_request(
    kind: EvidenceKind,
    service_name: &str,
) -> Result<CollectorRequest, CliError> {
    match kind {
        EvidenceKind::ServiceStatus | EvidenceKind::RecentLogs => {
            CollectorRequest::service(kind, service_name.to_owned()).map_err(CliError::from)
        }
        EvidenceKind::Disk | EvidenceKind::Process | EvidenceKind::Socket => {
            Ok(CollectorRequest::simple(kind))
        }
    }
}

fn run_dogfood_service_unhealthy(args: &[String]) -> Result<(), CliError> {
    let options = DogfoodOptions::parse(args)?;
    let backend = require_linux_dogfood_backend()?;
    let run_id = "run-real-host-service-unhealthy";
    let incident_id = "incident-real-host-service-unhealthy";
    let mut ledger = dogfood_incident_ledger(run_id, incident_id, &options.node_id)?;
    let plan = plan_dogfood_runbook(backend, run_id, &options)?;
    let evidence = collect_dogfood_evidence(backend, &options.service_name, &mut ledger, run_id)?;
    let proposal = build_dogfood_proposal(&mut ledger, run_id, &options, &evidence)?;
    let restart_task = dogfood_restart_task(&plan)?;
    let allowlist_entry_id = format!("allow-{}-restart", options.service_name);
    let claims = approve_dogfood_restart(
        &mut ledger,
        run_id,
        &proposal,
        restart_task,
        &options.service_name,
        &allowlist_entry_id,
    )?;
    validate_dogfood_helper(&mut ledger, run_id, &claims, &options, &allowlist_entry_id)?;
    record_dogfood_verification(&mut ledger, run_id, restart_task)?;
    let (receipt_text, state_root) = persist_dogfood_receipt(&mut ledger, run_id, &options)?;

    println!(
        "runlane-agent dogfood-service-unhealthy; mode=real-host-dry-run; os={:?}; node={}; service={}; evidence={}; helper=dry-run-validated; state_dir={}",
        backend.os(),
        options.node_id,
        options.service_name,
        evidence.len(),
        state_root.display()
    );
    println!("{receipt_text}");

    Ok(())
}

const DOGFOOD_APPROVAL_ID: &str = "approval-real-host-service-unhealthy";
const DOGFOOD_APPROVED_AT: u64 = 1_780_000_001;
const DOGFOOD_REQUESTED_AT: u64 = 1_780_000_000;
const DOGFOOD_EXPIRES_AT: u64 = 4_102_444_800;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DogfoodOptions {
    service_name: String,
    state_dir: PathBuf,
    node_id: String,
}

impl DogfoodOptions {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut flags = FlagParser::new(args);
        let service_name = flags.required("--service")?;
        let state_dir = flags.required_path("--state-dir")?;
        let node_id = flags
            .optional("--node-id")?
            .unwrap_or_else(|| "prod-web-01".to_owned());
        flags.finish()?;
        Ok(Self {
            service_name,
            state_dir,
            node_id,
        })
    }
}

fn require_linux_dogfood_backend() -> Result<NativeBackend, CliError> {
    let backend = NativeBackend::current();
    if matches!(backend.os(), runlane_core::OperatingSystem::Linux) {
        Ok(backend)
    } else {
        Err(CliError::Dogfood(
            "real-host service-unhealthy dogfood currently requires Linux/systemd".to_owned(),
        ))
    }
}

fn dogfood_incident_ledger(
    run_id: &str,
    incident_id: &str,
    node_id: &str,
) -> Result<AuditLedger, CliError> {
    let mut ledger = AuditLedger::empty();
    append_dogfood_event(
        &mut ledger,
        run_id,
        AuditEventKind::IncidentCreated {
            incident_id: incident_id.to_owned(),
            node_id: node_id.to_owned(),
            runbook: "service-unhealthy".to_owned(),
        },
    )?;
    Ok(ledger)
}

fn plan_dogfood_runbook(
    backend: NativeBackend,
    run_id: &str,
    options: &DogfoodOptions,
) -> Result<ServiceUnhealthyRunbookPlan, CliError> {
    let report = backend.capability_report(&options.node_id);
    runlane_core::plan_service_unhealthy_runbook(
        &ServiceUnhealthyRunbookRequest::new(run_id, &options.node_id, &options.service_name),
        &report,
    )
    .map_err(|error| CliError::Dogfood(format!("planning failed: {error:?}")))
}

fn collect_dogfood_evidence(
    backend: NativeBackend,
    service_name: &str,
    ledger: &mut AuditLedger,
    run_id: &str,
) -> Result<Vec<EvidenceEnvelope>, CliError> {
    let mut evidence = Vec::new();
    for kind in service_unhealthy_collector_kinds() {
        let request = service_unhealthy_collector_request(kind, service_name)?;
        let envelope = backend.collect_native(&request)?;
        append_dogfood_event(
            ledger,
            run_id,
            AuditEventKind::EvidenceCollected {
                source: envelope.source.clone(),
            },
        )?;
        evidence.push(envelope);
    }
    Ok(evidence)
}

fn build_dogfood_proposal(
    ledger: &mut AuditLedger,
    run_id: &str,
    options: &DogfoodOptions,
    evidence: &[EvidenceEnvelope],
) -> Result<StructuredProposal, CliError> {
    let proposal = analyze_service_unhealthy(
        "proposal-real-host-service-unhealthy",
        &options.node_id,
        &options.service_name,
        evidence,
    );
    validate_proposal(&proposal, &ProposalPolicy::service_unhealthy())
        .map_err(|error| CliError::Dogfood(format!("proposal validation failed: {error:?}")))?;
    if !proposal_has_service_restart(&proposal) {
        return Err(CliError::Dogfood(
            "real-host evidence did not produce a typed service.restart proposal".to_owned(),
        ));
    }
    append_dogfood_event(
        ledger,
        run_id,
        AuditEventKind::ProposalGenerated {
            proposal_id: proposal.id.clone(),
            hypothesis: proposal.hypothesis.clone(),
        },
    )?;
    Ok(proposal)
}

fn proposal_has_service_restart(proposal: &StructuredProposal) -> bool {
    proposal
        .proposed_actions
        .iter()
        .any(|action| matches!(action.kind, ProposedActionKind::ServiceRestart))
}

fn dogfood_restart_task(plan: &ServiceUnhealthyRunbookPlan) -> Result<&Task, CliError> {
    plan.run
        .tasks
        .iter()
        .find(|task| task.id == "restart-service")
        .ok_or_else(|| CliError::Dogfood("restart-service task missing".to_owned()))
}

fn approve_dogfood_restart(
    ledger: &mut AuditLedger,
    run_id: &str,
    proposal: &StructuredProposal,
    restart_task: &Task,
    service_name: &str,
    allowlist_entry_id: &str,
) -> Result<CapabilityLeaseClaims, CliError> {
    let mut approvals = ApprovalStore::empty();
    approvals
        .request(ApprovalRecord::new(
            DOGFOOD_APPROVAL_ID,
            run_id,
            proposal,
            "restart-service",
            runlane_core::OperationalLayer::System,
            service_name,
            restart_task.impact.clone(),
            restart_task.verification.clone(),
            DOGFOOD_REQUESTED_AT,
            DOGFOOD_EXPIRES_AT,
        ))
        .map_err(|error| CliError::Dogfood(format!("approval request failed: {error:?}")))?;
    let claims = approvals
        .approve(
            DOGFOOD_APPROVAL_ID,
            "restart-service",
            "real-host-dogfood-operator",
            DOGFOOD_APPROVED_AT,
            allowlist_entry_id,
            "real-host-dogfood-lease-nonce",
        )
        .map_err(|error| CliError::Dogfood(format!("approval failed: {error:?}")))?;
    for event in approvals.ledger.events() {
        append_dogfood_event(ledger, run_id, event.kind.clone())?;
    }
    append_dogfood_event(
        ledger,
        run_id,
        AuditEventKind::CapabilityLeaseIssued {
            lease_id: claims.lease_id.clone(),
            approval_id: claims.approval_id.clone(),
        },
    )?;
    append_dogfood_event(
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

fn validate_dogfood_helper(
    ledger: &mut AuditLedger,
    run_id: &str,
    claims: &CapabilityLeaseClaims,
    options: &DogfoodOptions,
    allowlist_entry_id: &str,
) -> Result<(), CliError> {
    let signed = SignedCapabilityLease::new(
        claims.clone(),
        "real-host-dogfood-key",
        "real-host-dogfood-signature",
    );
    let request = HelperActionRequest::new(
        &claims.lease_id,
        ActionKind::ServiceRestart,
        ActionTarget::new(&claims.target.resource_id, &options.service_name),
        [HelperArgument::new("service", &options.service_name)],
    );
    let allowlist = HelperAllowlist::new([HelperAllowlistEntry::new(
        allowlist_entry_id,
        ActionKind::ServiceRestart,
        &claims.target.resource_id,
    )]);
    validate_helper_request(
        &request,
        &signed,
        &HelperValidationContext::new(
            &options.node_id,
            DOGFOOD_APPROVED_AT,
            LeaseSignatureStatus::Valid,
            [],
            allowlist,
        ),
    )
    .map_err(|error| CliError::Dogfood(format!("helper dry-run validation failed: {error:?}")))?;
    append_dogfood_event(
        ledger,
        run_id,
        AuditEventKind::ActionResult {
            action: ActionKind::ServiceRestart,
            target: ActionTarget::new(&claims.target.resource_id, &options.service_name),
            status: HelperActionStatus::Succeeded,
        },
    )
}

fn record_dogfood_verification(
    ledger: &mut AuditLedger,
    run_id: &str,
    restart_task: &Task,
) -> Result<(), CliError> {
    let verification = VerificationPlanner::new(VerificationPolicy::minimal())
        .plan(&ActionKind::ServiceRestart, &restart_task.impact)
        .with_skipped(restart_task.verification.skipped.clone());
    append_dogfood_event(
        ledger,
        run_id,
        AuditEventKind::VerificationSelected {
            plan: verification.clone(),
        },
    )?;
    for check in &verification.required {
        append_dogfood_event(
            ledger,
            run_id,
            AuditEventKind::VerificationCompleted {
                check_id: check.id.clone(),
                resource_id: check.resource_id.clone(),
            },
        )?;
    }
    for skipped in &verification.skipped {
        append_dogfood_event(
            ledger,
            run_id,
            AuditEventKind::VerificationSkipped {
                skipped: skipped.clone(),
            },
        )?;
    }
    Ok(())
}

fn persist_dogfood_receipt(
    ledger: &mut AuditLedger,
    run_id: &str,
    options: &DogfoodOptions,
) -> Result<(String, PathBuf), CliError> {
    append_dogfood_event(
        ledger,
        run_id,
        AuditEventKind::CognitiveReceiptGenerated {
            receipt_id: "receipt-real-host-service-unhealthy".to_owned(),
        },
    )?;
    let receipt = generate_operator_receipt(run_id, ledger)
        .map_err(|error| CliError::Dogfood(format!("receipt generation failed: {error:?}")))?;
    let state = LocalServerState::init(&options.state_dir)
        .map_err(|error| CliError::Dogfood(format!("state init failed: {error}")))?;
    state
        .append_ledger(ledger)
        .map_err(|error| CliError::Dogfood(format!("state ledger append failed: {error}")))?;
    Ok((receipt.render_text(), state.layout.root))
}

fn current_supported_platform() -> Result<runlane_core::OperatingSystem, CliError> {
    let os = NativeBackend::current().os();
    if matches!(os, runlane_core::OperatingSystem::Unknown) {
        return Err(CliError::Config(AgentConfigError::InvalidField {
            field: "platform_family",
            reason: "detected platform is unsupported; pass a first-class v0.1 platform explicitly"
                .to_owned(),
        }));
    }
    Ok(os)
}

fn demo_enroll_pull() {
    let backend = NativeBackend::current();
    let report = backend.capability_report("local-node");
    let mut server = ControlPlane::empty();
    server
        .create_enrollment_token(EnrollmentToken::new(
            "token-local-node",
            "demo-token",
            "local-node",
            report.os,
            "demo-trust-root",
            200,
            "enroll-nonce",
        ))
        .expect("demo enrollment token is valid");
    let agent = server
        .enroll_agent(&AgentEnrollmentRequest::new(
            "demo-token",
            "local-node",
            report.os,
            "demo-cert-fingerprint",
            "demo-trust-root",
            100,
        ))
        .expect("demo agent enrolls");
    server.enqueue_task(PendingAgentTask::new(
        AgentTaskEnvelope::new(
            "env-local-1",
            "run-local-1",
            "collect-service-status",
            "local-node",
            100,
            200,
            "task-nonce",
            [Capability::new("service.systemd")],
            "audit-local-1",
        ),
        TypedTaskPayload::CollectEvidence {
            capability: Capability::new("service.systemd"),
            resource_id: "system:node/local-node/service/sshd".to_owned(),
        },
    ));
    let pulled = server
        .pull_task(&agent.identity(), 101)
        .expect("demo task pull succeeds");
    println!(
        "runlane-agent demo-enroll-pull; node={}; os={:?}; envelope={}; task={}; payload={:?}",
        agent.node_id,
        agent.os,
        pulled.envelope.envelope_id,
        pulled.envelope.task_id,
        pulled.payload
    );
}

fn service_status_fixture(os: runlane_core::OperatingSystem) -> &'static str {
    match os {
        runlane_core::OperatingSystem::Linux => {
            include_str!("../fixtures/linux/systemctl-status.txt")
        }
        runlane_core::OperatingSystem::FreeBsd => {
            include_str!("../fixtures/freebsd/service-status.txt")
        }
        runlane_core::OperatingSystem::OpenBsd => {
            include_str!("../fixtures/openbsd/rcctl-check.txt")
        }
        _ => "",
    }
}

#[derive(Debug)]
enum CliError {
    Config(AgentConfigError),
    Collector(CollectorExecutionError),
    Dogfood(String),
    Usage,
}

impl CliError {
    const fn usage() -> Self {
        Self::Usage
    }
}

impl From<AgentConfigError> for CliError {
    fn from(value: AgentConfigError) -> Self {
        Self::Config(value)
    }
}

impl From<CollectorExecutionError> for CliError {
    fn from(value: CollectorExecutionError) -> Self {
        Self::Collector(value)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => write!(f, "{error}"),
            Self::Collector(error) => write!(f, "{error}"),
            Self::Dogfood(message) => f.write_str(message),
            Self::Usage => write!(
                f,
                "usage: runlane-agent [demo-enroll-pull | config init|show|validate | identity install | collect-smoke --service <name> | dogfood-service-unhealthy --service <name> --state-dir <path> [--node-id <id>] | run]"
            ),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Collector(error) => Some(error),
            Self::Dogfood(_) | Self::Usage => None,
        }
    }
}

fn append_dogfood_event(
    ledger: &mut AuditLedger,
    run_id: &str,
    kind: AuditEventKind,
) -> Result<(), CliError> {
    let sequence = ledger.next_sequence();
    ledger
        .append(AuditEvent::new(
            format!("dogfood-event-{sequence}"),
            run_id,
            sequence,
            kind,
        ))
        .map_err(|error| CliError::Dogfood(format!("audit append failed: {error:?}")))
}

struct FlagParser {
    args: Vec<String>,
}

impl FlagParser {
    fn new(args: &[String]) -> Self {
        Self {
            args: args.to_vec(),
        }
    }

    fn required(&mut self, flag: &'static str) -> Result<String, CliError> {
        self.optional(flag)?.ok_or(CliError::Usage)
    }

    fn required_path(&mut self, flag: &'static str) -> Result<PathBuf, CliError> {
        self.required(flag).map(PathBuf::from)
    }

    fn required_u64(&mut self, flag: &'static str) -> Result<u64, CliError> {
        let value = self.required(flag)?;
        value.parse::<u64>().map_err(|_| {
            CliError::Config(AgentConfigError::InvalidField {
                field: flag,
                reason: format!("expected unsigned integer, got {value:?}"),
            })
        })
    }

    fn optional(&mut self, flag: &'static str) -> Result<Option<String>, CliError> {
        let Some(index) = self.args.iter().position(|arg| arg == flag) else {
            return Ok(None);
        };
        self.args.remove(index);
        if index >= self.args.len() || self.args[index].starts_with("--") {
            return Err(CliError::Usage);
        }
        Ok(Some(self.args.remove(index)))
    }

    fn optional_u64(&mut self, flag: &'static str) -> Result<Option<u64>, CliError> {
        self.optional(flag)?
            .map(|value| {
                value.parse::<u64>().map_err(|_| {
                    CliError::Config(AgentConfigError::InvalidField {
                        field: flag,
                        reason: format!("expected unsigned integer, got {value:?}"),
                    })
                })
            })
            .transpose()
    }

    fn present(&mut self, flag: &'static str) -> bool {
        if let Some(index) = self.args.iter().position(|arg| arg == flag) {
            self.args.remove(index);
            true
        } else {
            false
        }
    }

    fn finish(self) -> Result<(), CliError> {
        if self.args.is_empty() {
            Ok(())
        } else {
            Err(CliError::Usage)
        }
    }
}
