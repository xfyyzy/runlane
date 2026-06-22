use std::{env, process};

use runlane_core::{
    ApprovalOutcome, AuditEventKind,
    approval::{ApprovalState, demo_approval_store},
    e2e::{run_disk_pressure_simulation, run_service_unhealthy_simulation},
    fleet::FleetRepository,
    telegram::{
        TelegramApprovalAdapter, TelegramApprovalCommand, TelegramApprovalContext,
        TelegramApprovalError, TelegramApprovalResponse, TelegramAuthorizedActor, TelegramIdentity,
        TelegramIdentityMap,
    },
};

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.as_slice() {
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
        [approval, list] if approval == "approval" && list == "list" => {
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
            Ok(())
        }
        [approval, show, id] if approval == "approval" && show == "show" => {
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
        [approval, approve, id] if approval == "approval" && approve == "approve" => {
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
        [approval, reject, id] if approval == "approval" && reject == "reject" => {
            let mut store = demo_approval_store();
            store
                .reject(id, "cli-operator", 150)
                .map_err(|error| format!("rejection failed: {error:?}"))?;
            println!("rejected: {id}");
            Ok(())
        }
        [telegram, approval, smoke]
            if telegram == "telegram"
                && approval == "approval"
                && smoke == "live-simulated-smoke" =>
        {
            run_telegram_approval_live_simulated_smoke()
        }
        [demo, service, path] if demo == "demo" && service == "service-unhealthy" => {
            let simulation =
                run_service_unhealthy_simulation(path).map_err(|error| error.to_string())?;
            println!("run: {}", simulation.run_id);
            println!(
                "stages: {}",
                simulation
                    .stages
                    .iter()
                    .map(|stage| format!("{stage:?}"))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
            println!("{}", simulation.receipt.render_text());
            Ok(())
        }
        [demo, scenario, path] if demo == "demo" && scenario == "disk-pressure" => {
            let simulation =
                run_disk_pressure_simulation(path).map_err(|error| error.to_string())?;
            println!("run: {}", simulation.run_id);
            println!(
                "stages: {}",
                simulation
                    .stages
                    .iter()
                    .map(|stage| format!("{stage:?}"))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
            println!("{}", simulation.receipt.render_text());
            Ok(())
        }
        [receipt, show, id, path] if receipt == "receipt" && show == "show" => {
            if id == "run-demo-service-unhealthy" {
                let simulation =
                    run_service_unhealthy_simulation(path).map_err(|error| error.to_string())?;
                println!("{}", simulation.receipt.render_text());
                return Ok(());
            }
            if id == "run-demo-disk-pressure" {
                let simulation =
                    run_disk_pressure_simulation(path).map_err(|error| error.to_string())?;
                println!("{}", simulation.receipt.render_text());
                return Ok(());
            }
            Err(format!("unknown receipt: {id}"))
        }
        _ => Err(format!("unsupported runlane command: {}", args.join(" "))),
    }
}

fn print_help() {
    println!(
        "runlane commands:\n  runlane fleet validate <path>\n  runlane server gitops sync <path>\n  runlane approval list\n  runlane approval show <id>\n  runlane approval approve <id>\n  runlane approval reject <id>\n  runlane telegram approval live-simulated-smoke\n  runlane demo service-unhealthy <fleet-path>\n  runlane demo disk-pressure <fleet-path>\n  runlane receipt show <id> <fleet-path>"
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
    let chat_id = env_i64("RUNLANE_TELEGRAM_SMOKE_CHAT_ID", 42)?;
    let user_id = env_i64("RUNLANE_TELEGRAM_SMOKE_USER_ID", 1001)?;
    let unknown_user_id = env_i64(
        "RUNLANE_TELEGRAM_SMOKE_UNKNOWN_USER_ID",
        user_id
            .checked_add(1)
            .ok_or_else(|| "RUNLANE_TELEGRAM_SMOKE_USER_ID is too large".to_owned())?,
    )?;
    let username = env::var("RUNLANE_TELEGRAM_SMOKE_USERNAME").ok();
    let actor = env::var("RUNLANE_TELEGRAM_SMOKE_ACTOR")
        .unwrap_or_else(|_| "telegram:smoke-operator".to_owned());

    let adapter =
        TelegramApprovalAdapter::new(TelegramIdentityMap::new([TelegramAuthorizedActor::new(
            chat_id, user_id, actor,
        )]));
    let identity = TelegramIdentity::new(chat_id, user_id, username);
    let unknown_identity = TelegramIdentity::new(chat_id, unknown_user_id, None);
    let context = TelegramApprovalContext::new(
        150,
        "allow-prod-web-sshd-restart",
        "telegram-live-simulated-lease-nonce",
    );

    println!("telegram approval smoke mode: live-simulated");
    println!("secrets: not-read");
    println!("identity: redacted");

    let mut list_store = demo_approval_store();
    let listed = adapter
        .handle_command(
            &mut list_store,
            &identity,
            TelegramApprovalCommand::parse("/approvals").map_err(|error| format!("{error:?}"))?,
            &context,
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
            &identity,
            TelegramApprovalCommand::parse("/approval show approval-demo-1")
                .map_err(|error| format!("{error:?}"))?,
            &context,
        )
        .map_err(|error| format!("show failed: {error:?}"))?;
    match shown {
        TelegramApprovalResponse::ApprovalDetail(detail) => println!(
            "show: approval={} required_checks={} skipped_checks={}",
            detail.summary.id, detail.required_checks, detail.skipped_checks
        ),
        other => return Err(format!("show returned unexpected response: {other:?}")),
    }

    let mut approve_store = demo_approval_store();
    let approved = adapter
        .handle_command(
            &mut approve_store,
            &identity,
            TelegramApprovalCommand::parse("/approve approval-demo-1")
                .map_err(|error| format!("{error:?}"))?,
            &context,
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

    let mut reject_store = demo_approval_store();
    let rejected = adapter
        .handle_command(
            &mut reject_store,
            &identity,
            TelegramApprovalCommand::parse("/reject approval-demo-1")
                .map_err(|error| format!("{error:?}"))?,
            &context,
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

    let mut unknown_store = demo_approval_store();
    let unauthorized = adapter
        .handle_command(
            &mut unknown_store,
            &unknown_identity,
            TelegramApprovalCommand::List,
            &context,
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
    println!("telegram approval live-simulated smoke ok");

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
