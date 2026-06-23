use std::{env, process};

use runlane_core::{
    ApprovalOutcome, AuditEventKind,
    approval::{ApprovalState, demo_approval_store},
    durable::LocalServerState,
    e2e::{run_disk_pressure_simulation, run_service_unhealthy_simulation},
    fleet::FleetRepository,
    telegram::{
        TelegramApprovalAdapter, TelegramApprovalCommand, TelegramApprovalContext,
        TelegramApprovalError, TelegramApprovalResponse, TelegramAuthorizedActor, TelegramIdentity,
        TelegramIdentityMap,
    },
};

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = run(&args) {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args {
        [] => {
            print_help();
            Ok(())
        }
        [help] if help == "--help" || help == "-h" => {
            print_help();
            Ok(())
        }
        [fleet, validate, path] if fleet == "fleet" && validate == "validate" => {
            let repository = FleetRepository::load(path).map_err(|error| error.to_string())?;
            print_fleet_summary("fleet validation ok", &repository);
            Ok(())
        }
        [server, gitops, sync, path]
            if server == "server" && gitops == "gitops" && sync == "sync" =>
        {
            let repository = FleetRepository::load(path).map_err(|error| error.to_string())?;
            print_fleet_summary("server gitops sync ok", &repository);
            Ok(())
        }
        [approval, rest @ ..] if approval == "approval" => run_approval_command(rest),
        [telegram, approval, smoke]
            if telegram == "telegram"
                && approval == "approval"
                && smoke == "live-simulated-smoke" =>
        {
            run_telegram_approval_live_simulated_smoke()
        }
        [demo, rest @ ..] if demo == "demo" => run_demo_command(rest),
        [receipt, rest @ ..] if receipt == "receipt" => run_receipt_command(rest),
        _ => Err(format!("unsupported runlane command: {}", args.join(" "))),
    }
}

fn run_approval_command(args: &[String]) -> Result<(), String> {
    match args {
        [list] if list == "list" => {
            list_approvals();
            Ok(())
        }
        [show, id] if show == "show" => show_approval(id),
        [approve, id] if approve == "approve" => approve_demo(id),
        [reject, id] if reject == "reject" => reject_demo(id),
        _ => Err(format!("unsupported approval command: {}", args.join(" "))),
    }
}

fn list_approvals() {
    let store = demo_approval_store();
    for record in store.list_pending() {
        println!(
            "{} {} {:?} {} expires_at={}",
            record.id,
            record.action_id,
            record.action,
            record.target.resource_id,
            record.expires_at_unix_seconds
        );
    }
}

fn show_approval(id: &str) -> Result<(), String> {
    let store = demo_approval_store();
    let record = store
        .show(id)
        .ok_or_else(|| format!("unknown approval: {id}"))?;
    println!(
        "id: {}\nrun: {}\nproposal: {}\naction: {}\nlayer: {:?}\ntarget: {}\nlease: {:?}\nrequired_checks: {}\nskipped_checks: {}",
        record.id,
        record.run_id,
        record.proposal_id,
        record.action_id,
        record.layer,
        record.target.resource_id,
        record.lease_request.mode,
        record.verification.required.len(),
        record.verification.skipped.len()
    );
    Ok(())
}

fn approve_demo(id: &str) -> Result<(), String> {
    let mut store = demo_approval_store();
    let action_id = store
        .show(id)
        .ok_or_else(|| format!("unknown approval: {id}"))?
        .action_id
        .clone();
    let claims = store
        .approve(
            id,
            &action_id,
            "cli-operator",
            150,
            "allow-prod-web-sshd-restart",
            "cli-lease-nonce",
        )
        .map_err(|error| format!("approval failed: {error:?}"))?;
    println!(
        "approved: {}\nlease_id: {}\naction: {:?}\ntarget: {}",
        id, claims.lease_id, claims.action, claims.target.resource_id
    );
    Ok(())
}

fn reject_demo(id: &str) -> Result<(), String> {
    let mut store = demo_approval_store();
    store
        .reject(id, "cli-operator", 150)
        .map_err(|error| format!("rejection failed: {error:?}"))?;
    println!("rejected: {id}");
    Ok(())
}

fn run_demo_command(args: &[String]) -> Result<(), String> {
    match args {
        [service, path] if service == "service-unhealthy" => {
            print_service_unhealthy_simulation(path)
        }
        [scenario, path] if scenario == "disk-pressure" => print_disk_pressure_simulation(path),
        _ => Err(format!("unsupported demo command: {}", args.join(" "))),
    }
}

fn print_service_unhealthy_simulation(path: &str) -> Result<(), String> {
    let simulation = run_service_unhealthy_simulation(path).map_err(|error| error.to_string())?;
    print_simulation(
        &simulation.run_id,
        &simulation.stages,
        &simulation.receipt.render_text(),
    );
    Ok(())
}

fn print_disk_pressure_simulation(path: &str) -> Result<(), String> {
    let simulation = run_disk_pressure_simulation(path).map_err(|error| error.to_string())?;
    print_simulation(
        &simulation.run_id,
        &simulation.stages,
        &simulation.receipt.render_text(),
    );
    Ok(())
}

fn print_simulation(run_id: &str, stages: &[runlane_core::e2e::JourneyStage], receipt: &str) {
    println!("run: {run_id}");
    println!(
        "stages: {}",
        stages
            .iter()
            .map(|stage| format!("{stage:?}"))
            .collect::<Vec<_>>()
            .join(" -> ")
    );
    println!("{receipt}");
}

fn run_receipt_command(args: &[String]) -> Result<(), String> {
    match args {
        [show, id, path] if show == "show" => show_receipt(id, path),
        _ => Err(format!("unsupported receipt command: {}", args.join(" "))),
    }
}

fn show_receipt(id: &str, path: &str) -> Result<(), String> {
    if id == "run-demo-service-unhealthy" {
        let simulation =
            run_service_unhealthy_simulation(path).map_err(|error| error.to_string())?;
        println!("{}", simulation.receipt.render_text());
        return Ok(());
    }
    if id == "run-demo-disk-pressure" {
        let simulation = run_disk_pressure_simulation(path).map_err(|error| error.to_string())?;
        println!("{}", simulation.receipt.render_text());
        return Ok(());
    }
    let state = LocalServerState::open(path);
    let receipt = state
        .render_receipt(id)
        .map_err(|error| format!("unknown receipt or unreadable state for {id}: {error}"))?;
    println!("{}", receipt.render_text());
    Ok(())
}

fn print_help() {
    println!(
        "runlane commands:\n  runlane fleet validate <path>\n  runlane server gitops sync <path>\n  runlane approval list\n  runlane approval show <id>\n  runlane approval approve <id>\n  runlane approval reject <id>\n  runlane telegram approval live-simulated-smoke\n  runlane demo service-unhealthy <fleet-path>\n  runlane demo disk-pressure <fleet-path>\n  runlane receipt show <id> <fleet-or-state-path>"
    );
}

fn print_fleet_summary(prefix: &str, repository: &FleetRepository) {
    let summary = repository.summary();
    println!(
        "{prefix}; nodes={}; roles={}; runbooks={}; policies={}; allowlists={}; overlays={}; resolved_settings={}",
        summary.nodes,
        summary.roles,
        summary.runbooks,
        summary.policies,
        summary.allowlists,
        summary.overlays,
        summary.resolved_settings
    );
}

fn run_telegram_approval_live_simulated_smoke() -> Result<(), String> {
    let input = TelegramSmokeInput::load()?;
    let adapter =
        TelegramApprovalAdapter::new(TelegramIdentityMap::new([TelegramAuthorizedActor::new(
            input.chat_id,
            input.user_id,
            input.actor,
        )]));
    let identity = TelegramIdentity::new(input.chat_id, input.user_id, input.username);
    let unknown_identity = TelegramIdentity::new(input.chat_id, input.unknown_user_id, None);
    let context = TelegramApprovalContext::new(
        150,
        "allow-prod-web-sshd-restart",
        "telegram-live-simulated-lease-nonce",
    );

    println!("telegram approval smoke mode: live-simulated");
    println!("secrets: not-read");
    println!("identity: redacted");

    smoke_list_and_show_approval(&adapter, &identity, &context)?;
    smoke_approve_approval(&adapter, &identity, &context)?;
    smoke_reject_approval(&adapter, &identity, &context)?;
    smoke_unknown_identity(&adapter, &unknown_identity, &context)?;
    println!("telegram approval live-simulated smoke ok");

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TelegramSmokeInput {
    chat_id: i64,
    user_id: i64,
    unknown_user_id: i64,
    username: Option<String>,
    actor: String,
}

impl TelegramSmokeInput {
    fn load() -> Result<Self, String> {
        let chat_id = env_i64("RUNLANE_TELEGRAM_SMOKE_CHAT_ID", 42)?;
        let user_id = env_i64("RUNLANE_TELEGRAM_SMOKE_USER_ID", 1001)?;
        let unknown_user_id = env_i64(
            "RUNLANE_TELEGRAM_SMOKE_UNKNOWN_USER_ID",
            user_id
                .checked_add(1)
                .ok_or_else(|| "RUNLANE_TELEGRAM_SMOKE_USER_ID is too large".to_owned())?,
        )?;
        Ok(Self {
            chat_id,
            user_id,
            unknown_user_id,
            username: env::var("RUNLANE_TELEGRAM_SMOKE_USERNAME").ok(),
            actor: env::var("RUNLANE_TELEGRAM_SMOKE_ACTOR")
                .unwrap_or_else(|_| "telegram:smoke-operator".to_owned()),
        })
    }
}

fn smoke_list_and_show_approval(
    adapter: &TelegramApprovalAdapter,
    identity: &TelegramIdentity,
    context: &TelegramApprovalContext,
) -> Result<(), String> {
    let mut list_store = demo_approval_store();
    let listed = adapter
        .handle_command(
            &mut list_store,
            identity,
            TelegramApprovalCommand::parse("/approvals").map_err(|error| format!("{error:?}"))?,
            context,
        )
        .map_err(|error| format!("list failed: {error:?}"))?;
    let pending_count = match listed {
        TelegramApprovalResponse::PendingApprovals(pending) => pending.len(),
        other => return Err(format!("list returned unexpected response: {other:?}")),
    };
    println!("list: pending={pending_count}");

    let shown = adapter
        .handle_command(
            &mut list_store,
            identity,
            TelegramApprovalCommand::parse("/approval show approval-demo-1")
                .map_err(|error| format!("{error:?}"))?,
            context,
        )
        .map_err(|error| format!("show failed: {error:?}"))?;
    match shown {
        TelegramApprovalResponse::ApprovalDetail(detail) => println!(
            "show: approval={} required_checks={} skipped_checks={}",
            detail.summary.id, detail.required_checks, detail.skipped_checks
        ),
        other => return Err(format!("show returned unexpected response: {other:?}")),
    }
    Ok(())
}

fn smoke_approve_approval(
    adapter: &TelegramApprovalAdapter,
    identity: &TelegramIdentity,
    context: &TelegramApprovalContext,
) -> Result<(), String> {
    let mut approve_store = demo_approval_store();
    let approved = adapter
        .handle_command(
            &mut approve_store,
            identity,
            TelegramApprovalCommand::parse("/approve approval-demo-1")
                .map_err(|error| format!("{error:?}"))?,
            context,
        )
        .map_err(|error| format!("approve failed: {error:?}"))?;
    match approved {
        TelegramApprovalResponse::Approved {
            approval_id,
            lease_id,
            ..
        } => println!(
            "approve: approval={} lease={} audit={}",
            approval_id,
            lease_id,
            approval_decision_audited(&approve_store, ApprovalOutcome::Approved)
        ),
        other => return Err(format!("approve returned unexpected response: {other:?}")),
    }
    Ok(())
}

fn smoke_reject_approval(
    adapter: &TelegramApprovalAdapter,
    identity: &TelegramIdentity,
    context: &TelegramApprovalContext,
) -> Result<(), String> {
    let mut reject_store = demo_approval_store();
    let rejected = adapter
        .handle_command(
            &mut reject_store,
            identity,
            TelegramApprovalCommand::parse("/reject approval-demo-1")
                .map_err(|error| format!("{error:?}"))?,
            context,
        )
        .map_err(|error| format!("reject failed: {error:?}"))?;
    match rejected {
        TelegramApprovalResponse::Rejected { approval_id, .. } => println!(
            "reject: approval={} audit={}",
            approval_id,
            approval_decision_audited(&reject_store, ApprovalOutcome::Rejected)
        ),
        other => return Err(format!("reject returned unexpected response: {other:?}")),
    }
    Ok(())
}

fn smoke_unknown_identity(
    adapter: &TelegramApprovalAdapter,
    unknown_identity: &TelegramIdentity,
    context: &TelegramApprovalContext,
) -> Result<(), String> {
    let mut unknown_store = demo_approval_store();
    let unauthorized = adapter
        .handle_command(
            &mut unknown_store,
            unknown_identity,
            TelegramApprovalCommand::List,
            context,
        )
        .expect_err("unknown identity should fail closed");
    if !matches!(
        unauthorized,
        TelegramApprovalError::UnauthorizedIdentity { .. }
    ) {
        return Err(format!(
            "unknown identity returned unexpected error: {unauthorized:?}"
        ));
    }
    let still_pending = unknown_store
        .show("approval-demo-1")
        .is_some_and(|record| record.state == ApprovalState::Pending);
    let audited = adapter_rejection_audited(&unknown_store);
    if !still_pending || !audited {
        return Err(format!(
            "unknown identity fail-closed evidence missing: pending={still_pending} audited={audited}"
        ));
    }
    println!("unknown_identity: rejected pending={still_pending} audit={audited}");
    Ok(())
}

fn env_i64(name: &str, default: i64) -> Result<i64, String> {
    match env::var(name) {
        Ok(value) => value
            .parse::<i64>()
            .map_err(|_| format!("{name} must be a signed integer")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(format!("{name}: {error}")),
    }
}

fn approval_decision_audited(
    store: &runlane_core::approval::ApprovalStore,
    outcome: ApprovalOutcome,
) -> bool {
    store.ledger.events().iter().any(|event| {
        matches!(
            &event.kind,
            AuditEventKind::ApprovalDecision {
                outcome: event_outcome,
                ..
            } if *event_outcome == outcome
        )
    })
}

fn adapter_rejection_audited(store: &runlane_core::approval::ApprovalStore) -> bool {
    store.ledger.events().iter().any(|event| {
        matches!(
            &event.kind,
            AuditEventKind::ApprovalAdapterRejected { adapter, reason }
                if adapter == "telegram" && reason == "unauthorized_identity"
        )
    })
}
