use std::{env, process};

use runlane_core::{
    approval::demo_approval_store, e2e::run_service_unhealthy_simulation, fleet::FleetRepository,
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
        [receipt, show, id, path] if receipt == "receipt" && show == "show" => {
            let simulation =
                run_service_unhealthy_simulation(path).map_err(|error| error.to_string())?;
            if simulation.run_id != *id {
                return Err(format!("unknown receipt: {id}"));
            }
            println!("{}", simulation.receipt.render_text());
            Ok(())
        }
        _ => Err(format!("unsupported runlane command: {}", args.join(" "))),
    }
}

fn print_help() {
    println!(
        "runlane commands:\n  runlane fleet validate <path>\n  runlane server gitops sync <path>\n  runlane approval list\n  runlane approval show <id>\n  runlane approval approve <id>\n  runlane approval reject <id>\n  runlane demo service-unhealthy <fleet-path>\n  runlane receipt show <id> <fleet-path>"
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
