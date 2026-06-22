use std::{env, process};

use runlane_core::{
    AgentResultStatus, AgentResultSubmission, AgentTaskEnvelope, Capability, OperatingSystem,
    RunState,
    durable::LocalServerState,
    e2e::run_service_unhealthy_simulation,
    runtime::{
        AgentEnrollmentRequest, ControlPlane, EnrollmentToken, PendingAgentTask, TypedTaskPayload,
        runtime_text_evidence,
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
            println!(
                "runlane-server skeleton; initial_run_state={:?}",
                RunState::Created
            );
            Ok(())
        }
        [demo] if demo == "demo-control-plane" => {
            demo_control_plane();
            Ok(())
        }
        [state, demo_write, state_dir, fleet_path]
            if state == "state" && demo_write == "demo-write" =>
        {
            let simulation =
                run_service_unhealthy_simulation(fleet_path).map_err(|error| error.to_string())?;
            let state = LocalServerState::init(state_dir).map_err(|error| error.to_string())?;
            state
                .append_ledger(&simulation.ledger)
                .map_err(|error| error.to_string())?;
            println!(
                "server state demo-write ok; run={}; events={}; ledger={}",
                simulation.run_id,
                simulation.ledger.events().len(),
                state.layout.audit_ledger.display()
            );
            Ok(())
        }
        [state, receipt, state_dir, run_id] if state == "state" && receipt == "receipt" => {
            let state = LocalServerState::open(state_dir);
            let receipt = state
                .render_receipt(run_id)
                .map_err(|error| error.to_string())?;
            println!("{}", receipt.render_text());
            Ok(())
        }
        _ => Err(format!(
            "unsupported runlane-server command: {}",
            args.join(" ")
        )),
    }
}

fn demo_control_plane() {
    let mut server = ControlPlane::empty();
    server
        .create_enrollment_token(EnrollmentToken::new(
            "token-prod-web-01",
            "demo-token",
            "prod-web-01",
            OperatingSystem::Linux,
            "demo-trust-root",
            200,
            "enroll-nonce",
        ))
        .expect("demo token is valid");
    let agent = server
        .enroll_agent(&AgentEnrollmentRequest::new(
            "demo-token",
            "prod-web-01",
            OperatingSystem::Linux,
            "demo-cert-fingerprint",
            "demo-trust-root",
            100,
        ))
        .expect("demo agent enrolls");
    server.enqueue_task(PendingAgentTask::new(
        AgentTaskEnvelope::new(
            "env-1",
            "run-1",
            "collect-service-status",
            "prod-web-01",
            100,
            200,
            "nonce-1",
            [Capability::new("service.systemd")],
            "audit-1",
        ),
        TypedTaskPayload::CollectEvidence {
            capability: Capability::new("service.systemd"),
            resource_id: "system:node/prod-web-01/service/sshd".to_owned(),
        },
    ));
    let pulled = server
        .pull_task(&agent.identity(), 101)
        .expect("demo pull succeeds");
    server
        .submit_result(
            &agent.identity(),
            AgentResultSubmission::new(
                &pulled.envelope.envelope_id,
                &pulled.envelope.run_id,
                &pulled.envelope.task_id,
                &pulled.envelope.node_id,
                &pulled.envelope.nonce,
                AgentResultStatus::Succeeded,
                [runtime_text_evidence("service_status", "sshd active")],
                &pulled.envelope.audit_correlation_id,
            ),
            102,
        )
        .expect("demo result accepted");
    println!(
        "runlane-server demo-control-plane; enrolled_agents={}; accepted_results={}; audit_events={}",
        server.agents().len(),
        server.accepted_results.len(),
        server.ledger.events().len()
    );
}
