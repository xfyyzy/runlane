use std::env;

use runlane_core::{
    AgentResultStatus, AgentResultSubmission, AgentTaskEnvelope, Capability, OperatingSystem,
    RunState,
    runtime::{
        AgentEnrollmentRequest, ControlPlane, EnrollmentToken, PendingAgentTask, TypedTaskPayload,
        runtime_text_evidence,
    },
};

fn main() {
    if env::args().nth(1).as_deref() == Some("demo-control-plane") {
        demo_control_plane();
        return;
    }

    println!(
        "runlane-server skeleton; initial_run_state={:?}",
        RunState::Created
    );
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
