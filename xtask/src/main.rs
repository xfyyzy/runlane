use std::{
    env,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
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
        [smoke] if smoke == "smoke" => {
            print_smoke_list();
            Ok(())
        }
        [smoke, help] if smoke == "smoke" && (help == "--help" || help == "-h") => {
            print_help();
            Ok(())
        }
        [smoke, list] if smoke == "smoke" && list == "list" => {
            print_smoke_list();
            Ok(())
        }
        [smoke, safe, rest @ ..] if smoke == "smoke" && safe == "safe" => {
            let options = RunOptions::parse(rest)?;
            if options.confirm_host_mutation {
                return Err("--confirm-host-mutation is not valid for smoke safe".to_owned());
            }
            if !options.passthrough.is_empty() {
                return Err("smoke safe does not accept passthrough script arguments".to_owned());
            }
            run_safe_smokes(options.dry_run)
        }
        [smoke, name, rest @ ..] if smoke == "smoke" => {
            let options = RunOptions::parse(rest)?;
            run_named_smoke(name, &options)
        }
        _ => Err(format!("unsupported xtask command: {}", args.join(" "))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Safety {
    Safe,
    HostMutating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Smoke {
    name: &'static str,
    safety: Safety,
    in_safe_suite: bool,
    summary: &'static str,
    side_effects: &'static [&'static str],
    steps: &'static [Step],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    program: &'static str,
    args: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunOptions {
    dry_run: bool,
    confirm_host_mutation: bool,
    passthrough: Vec<String>,
}

impl RunOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut dry_run = false;
        let mut confirm_host_mutation = false;
        let mut passthrough = Vec::new();
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--dry-run" => {
                    dry_run = true;
                    index += 1;
                }
                "--confirm-host-mutation" => {
                    confirm_host_mutation = true;
                    index += 1;
                }
                "--" => {
                    passthrough.extend(args[index + 1..].iter().cloned());
                    break;
                }
                other => return Err(format!("unsupported smoke option: {other}")),
            }
        }

        Ok(Self {
            dry_run,
            confirm_host_mutation,
            passthrough,
        })
    }
}

const SMOKES: &[Smoke] = &[
    Smoke {
        name: "fleet",
        safety: Safety::Safe,
        in_safe_suite: true,
        summary: "validate the example fleet and server GitOps ingest path",
        side_effects: &[],
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "runlane",
                    "--",
                    "fleet",
                    "validate",
                    "examples/fleet",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "runlane",
                    "--",
                    "server",
                    "gitops",
                    "sync",
                    "examples/fleet",
                ],
            },
        ],
    },
    Smoke {
        name: "control-plane",
        safety: Safety::Safe,
        in_safe_suite: true,
        summary: "exercise in-process server and agent enrollment/pull demos",
        side_effects: &[],
        steps: &[
            Step {
                program: "cargo",
                args: &["run", "-p", "runlane-server", "--", "demo-control-plane"],
            },
            Step {
                program: "cargo",
                args: &["run", "-p", "runlane-agent", "--", "demo-enroll-pull"],
            },
        ],
    },
    Smoke {
        name: "http",
        safety: Safety::Safe,
        in_safe_suite: true,
        summary: "run the loopback live HTTP transport smoke",
        side_effects: &["starts a temporary runlane-server on 127.0.0.1 and tears it down"],
        steps: &[Step {
            program: "scripts/smoke/live-http-transport.sh",
            args: &[],
        }],
    },
    Smoke {
        name: "telegram-live-simulated",
        safety: Safety::Safe,
        in_safe_suite: true,
        summary: "run Telegram approval adapter tests and live-simulated CLI smoke",
        side_effects: &["uses no Telegram token and reads no secrets"],
        steps: &[Step {
            program: "scripts/smoke/telegram-approval-live-simulated.sh",
            args: &[],
        }],
    },
    Smoke {
        name: "e2e",
        safety: Safety::Safe,
        in_safe_suite: true,
        summary: "run deterministic service-unhealthy and disk-pressure demos",
        side_effects: &[],
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "runlane",
                    "--",
                    "demo",
                    "service-unhealthy",
                    "examples/fleet",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "runlane",
                    "--",
                    "demo",
                    "disk-pressure",
                    "examples/fleet",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "runlane",
                    "--",
                    "receipt",
                    "show",
                    "run-demo-service-unhealthy",
                    "examples/fleet",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "runlane",
                    "--",
                    "receipt",
                    "show",
                    "run-demo-disk-pressure",
                    "examples/fleet",
                ],
            },
        ],
    },
    Smoke {
        name: "linux-helper-install",
        safety: Safety::HostMutating,
        in_safe_suite: false,
        summary: "install and validate the Linux sudo helper boundary",
        side_effects: &[
            "requires Linux, sudo -n, and an existing unprivileged smoke user",
            "installs a root-owned helper, allowlist, and sudoers fragment",
            "restores prior helper, allowlist, and sudoers state unless the script is told to keep it",
        ],
        steps: &[Step {
            program: "scripts/smoke/linux-helper-install.sh",
            args: &[],
        }],
    },
    Smoke {
        name: "linux-service-unhealthy-dogfood",
        safety: Safety::HostMutating,
        in_safe_suite: false,
        summary: "run the controlled Linux systemd service-unhealthy dogfood",
        side_effects: &[
            "requires Linux, systemd, and sudo -n",
            "creates only runlane-demo-unhealthy.service",
            "removes the controlled demo service and temporary state during teardown",
        ],
        steps: &[Step {
            program: "scripts/smoke/linux-service-unhealthy-dogfood.sh",
            args: &[],
        }],
    },
    Smoke {
        name: "freebsd-vm-validation",
        safety: Safety::HostMutating,
        in_safe_suite: false,
        summary: "run FreeBSD VM validation including sudo helper smoke",
        side_effects: &[
            "must run inside the intended FreeBSD VM",
            "uses sudo to install helper, allowlist, and sudoers test state",
            "restores prior VM helper, allowlist, and sudoers state during teardown",
        ],
        steps: &[Step {
            program: "scripts/smoke/freebsd-vm-validation.sh",
            args: &[],
        }],
    },
    Smoke {
        name: "openbsd-vm-validation",
        safety: Safety::HostMutating,
        in_safe_suite: false,
        summary: "run OpenBSD VM validation including doas helper smoke",
        side_effects: &[
            "must run inside the intended OpenBSD VM",
            "uses doas to install helper, allowlist, and doas test state",
            "restores prior VM helper, allowlist, and doas state during teardown",
        ],
        steps: &[Step {
            program: "scripts/smoke/openbsd-vm-validation.sh",
            args: &[],
        }],
    },
];

fn print_help() {
    println!(
        "cargo xtask smoke list\ncargo xtask smoke safe [--dry-run]\ncargo xtask smoke <name> [--dry-run] [--confirm-host-mutation] [-- <script-args>]"
    );
}

fn print_smoke_list() {
    println!("safe suite:");
    for smoke in SMOKES.iter().filter(|smoke| smoke.in_safe_suite) {
        println!("  {:32} {}", smoke.name, smoke.summary);
    }
    println!();
    println!("explicit host-mutating or VM smokes:");
    for smoke in SMOKES
        .iter()
        .filter(|smoke| smoke.safety == Safety::HostMutating)
    {
        println!("  {:32} {}", smoke.name, smoke.summary);
        for effect in smoke.side_effects {
            println!("    side effect: {effect}");
        }
    }
}

fn run_safe_smokes(dry_run: bool) -> Result<(), String> {
    for smoke in SMOKES.iter().filter(|smoke| smoke.in_safe_suite) {
        run_smoke(smoke, dry_run, &[])?;
    }
    Ok(())
}

fn run_named_smoke(name: &str, options: &RunOptions) -> Result<(), String> {
    let smoke = SMOKES
        .iter()
        .find(|smoke| smoke.name == name)
        .ok_or_else(|| format!("unknown smoke: {name}; run cargo xtask smoke list"))?;
    if smoke.safety == Safety::HostMutating && !options.confirm_host_mutation && !options.dry_run {
        describe_host_mutation(smoke);
        return Err(format!(
            "refusing to run host-mutating smoke {name}; rerun with --confirm-host-mutation"
        ));
    }
    if !options.passthrough.is_empty() && smoke.steps.len() != 1 {
        return Err(format!(
            "smoke {name} has multiple steps; passthrough arguments are only supported for single-script smokes"
        ));
    }
    run_smoke(smoke, options.dry_run, &options.passthrough)
}

fn describe_host_mutation(smoke: &Smoke) {
    eprintln!("host-mutating smoke: {}", smoke.name);
    eprintln!("{}", smoke.summary);
    for effect in smoke.side_effects {
        eprintln!("side effect: {effect}");
    }
}

fn run_smoke(smoke: &Smoke, dry_run: bool, passthrough: &[String]) -> Result<(), String> {
    println!("smoke: {} ({})", smoke.name, smoke.summary);
    if !smoke.side_effects.is_empty() {
        for effect in smoke.side_effects {
            println!("side effect: {effect}");
        }
    }
    for step in smoke.steps {
        run_step(step, dry_run, passthrough)?;
    }
    Ok(())
}

fn run_step(step: &Step, dry_run: bool, passthrough: &[String]) -> Result<(), String> {
    let mut display_args = step
        .args
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    display_args.extend(passthrough.iter().cloned());
    if display_args.is_empty() {
        println!("$ {}", step.program);
    } else {
        println!("$ {} {}", step.program, display_args.join(" "));
    }
    if dry_run {
        return Ok(());
    }

    let status = Command::new(step.program)
        .args(step.args)
        .args(passthrough)
        .status()
        .map_err(|error| format!("failed to start {}: {error}", step.program))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} exited with status {}",
            step.program,
            status.code().map_or_else(
                || "terminated by signal".to_owned(),
                |code| code.to_string()
            )
        ))
    }
}
