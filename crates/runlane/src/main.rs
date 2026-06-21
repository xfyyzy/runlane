use std::{env, process};

use runlane_core::fleet::FleetRepository;

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
        _ => Err(format!("unsupported runlane command: {}", args.join(" "))),
    }
}

fn print_help() {
    println!(
        "runlane commands:\n  runlane fleet validate <path>\n  runlane server gitops sync <path>"
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
