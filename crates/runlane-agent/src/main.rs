mod platform;

use std::env;

use platform::{NativeBackend, PlatformBackend};
use runlane_core::{
    AgentTaskEnvelope, Capability,
    runtime::{
        AgentEnrollmentRequest, ControlPlane, EnrollmentToken, PendingAgentTask, TypedTaskPayload,
    },
};

fn main() {
    if env::args().nth(1).as_deref() == Some("demo-enroll-pull") {
        demo_enroll_pull();
        return;
    }

    let backend = NativeBackend::current();
    let report = backend.capability_report("local-node");
    let fixture_stub_count = backend.parser_fixture_stubs().len();
    let capability_probe_ok = report
        .capabilities
        .first()
        .is_some_and(|capability| backend.require_capability(capability).is_ok());
    println!(
        "runlane-agent skeleton; detected_os={:?}; capabilities={}; unsupported={}; fixture_stubs={}; capability_probe_ok={}",
        report.os,
        report.capabilities.len(),
        report.unsupported.len(),
        fixture_stub_count,
        capability_probe_ok
    );
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
