use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use runlane_core::{
    AgentIdentity, AgentResultStatus, AgentResultSubmission, EvidenceEnvelope, OperatingSystem,
    runtime::{AgentEnrollmentRequest, ControlPlane, EnrollmentToken, RuntimeError},
};
use serde::{Deserialize, Serialize};

const HEADER_NODE_ID: &str = "x-runlane-node-id";
const HEADER_CERT_FINGERPRINT: &str = "x-runlane-certificate-fingerprint";

#[derive(Clone)]
pub struct HttpState {
    control_plane: Arc<Mutex<ControlPlane>>,
}

impl HttpState {
    pub fn new(control_plane: ControlPlane) -> Self {
        Self {
            control_plane: Arc::new(Mutex::new(control_plane)),
        }
    }
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/v1/enrollment/tokens", post(create_enrollment_token))
        .route("/v1/agent/enroll", post(enroll_agent))
        .route("/v1/agent/pull", post(pull_task))
        .route("/v1/agent/result", post(submit_result))
        .route("/v1/agent/spool/replay", post(replay_spool))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct CreateEnrollmentTokenRequest {
    token_id: String,
    token: String,
    node_id: String,
    platform_family: String,
    server_trust_root: String,
    expires_at_unix_seconds: u64,
    nonce: String,
}

#[derive(Debug, Serialize)]
struct CreateEnrollmentTokenResponse {
    status: &'static str,
    token_id: String,
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct EnrollmentRequestDto {
    token: String,
    node_id: String,
    platform_family: String,
    certificate_fingerprint: String,
    server_trust_root: String,
    now_unix_seconds: u64,
}

#[derive(Debug, Serialize)]
struct AgentIdentityResponse {
    node_id: String,
    platform_family: &'static str,
    certificate_fingerprint: String,
    server_trust_root: String,
}

#[derive(Debug, Deserialize)]
struct PullTaskRequest {
    node_id: String,
    now_unix_seconds: u64,
    capability_report_version: Option<String>,
    last_seen_task_nonce: Option<String>,
}

#[derive(Debug, Serialize)]
struct PullTaskResponse {
    envelope: AgentTaskEnvelopeDto,
    payload: TypedTaskPayloadDto,
}

#[derive(Debug, Serialize)]
struct AgentTaskEnvelopeDto {
    envelope_id: String,
    run_id: String,
    task_id: String,
    node_id: String,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    nonce: String,
    required_capabilities: Vec<String>,
    audit_correlation_id: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TypedTaskPayloadDto {
    CollectEvidence {
        capability: String,
        resource_id: String,
    },
    ExecuteTypedHelperRequest {
        action: String,
        target_resource_id: String,
    },
    Verify {
        check_id: String,
        resource_id: String,
    },
}

#[derive(Debug, Deserialize)]
struct AgentResultSubmissionDto {
    envelope_id: String,
    run_id: String,
    task_id: String,
    node_id: String,
    nonce: String,
    status: String,
    #[serde(default)]
    now_unix_seconds: Option<u64>,
    evidence: Vec<EvidenceDto>,
    audit_correlation_id: String,
}

#[derive(Debug, Serialize)]
struct AgentResultResponse {
    status: &'static str,
    envelope_id: String,
    task_id: String,
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct EvidenceDto {
    source: String,
    content_type: String,
    body: String,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct SpoolReplayRequest {
    spool_id: String,
    reason: Option<String>,
    result: AgentResultSubmissionDto,
    now_unix_seconds: u64,
}

#[derive(Debug, Serialize)]
struct SpoolReplayResponse {
    status: &'static str,
    spool_id: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    kind: &'static str,
}

#[derive(Debug)]
struct HttpApiError {
    status: StatusCode,
    kind: &'static str,
    message: String,
}

impl HttpApiError {
    fn new(status: StatusCode, kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            kind,
            message: message.into(),
        }
    }

    fn bad_request(kind: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, kind, message)
    }

    fn unauthorized(kind: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, kind, message)
    }

    fn conflict(kind: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, kind, message)
    }
}

impl IntoResponse for HttpApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
                kind: self.kind,
            }),
        )
            .into_response()
    }
}

async fn create_enrollment_token(
    State(state): State<HttpState>,
    Json(request): Json<CreateEnrollmentTokenRequest>,
) -> Result<Json<CreateEnrollmentTokenResponse>, HttpApiError> {
    let os = parse_operating_system(&request.platform_family)?;
    let mut control_plane = state.lock()?;
    control_plane
        .create_enrollment_token(EnrollmentToken::new(
            &request.token_id,
            &request.token,
            &request.node_id,
            os,
            &request.server_trust_root,
            request.expires_at_unix_seconds,
            &request.nonce,
        ))
        .map_err(runtime_error)?;
    Ok(Json(CreateEnrollmentTokenResponse {
        status: "created",
        token_id: request.token_id,
        node_id: request.node_id,
    }))
}

async fn enroll_agent(
    State(state): State<HttpState>,
    Json(request): Json<EnrollmentRequestDto>,
) -> Result<Json<AgentIdentityResponse>, HttpApiError> {
    let os = parse_operating_system(&request.platform_family)?;
    let mut control_plane = state.lock()?;
    let identity = control_plane
        .enroll_agent(&AgentEnrollmentRequest::new(
            &request.token,
            &request.node_id,
            os,
            &request.certificate_fingerprint,
            &request.server_trust_root,
            request.now_unix_seconds,
        ))
        .map_err(runtime_error)?;
    Ok(Json(AgentIdentityResponse {
        node_id: identity.node_id,
        platform_family: format_operating_system(identity.os),
        certificate_fingerprint: identity.certificate_fingerprint,
        server_trust_root: identity.server_trust_root,
    }))
}

async fn pull_task(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(request): Json<PullTaskRequest>,
) -> Result<Json<PullTaskResponse>, HttpApiError> {
    let identity = identity_from_headers(&headers)?;
    if identity.node_id != request.node_id {
        return Err(HttpApiError::new(
            StatusCode::FORBIDDEN,
            "identity_node_mismatch",
            "request node_id does not match extracted agent identity",
        ));
    }
    let _capability_report_version = request.capability_report_version.as_deref();
    let _last_seen_task_nonce = request.last_seen_task_nonce.as_deref();
    let mut control_plane = state.lock()?;
    let pulled = control_plane
        .pull_task(&identity, request.now_unix_seconds)
        .map_err(runtime_error)?;
    Ok(Json(PullTaskResponse {
        envelope: AgentTaskEnvelopeDto::from(pulled.envelope),
        payload: TypedTaskPayloadDto::from(pulled.payload),
    }))
}

async fn submit_result(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(request): Json<AgentResultSubmissionDto>,
) -> Result<Json<AgentResultResponse>, HttpApiError> {
    let identity = identity_from_headers(&headers)?;
    let now_unix_seconds = request.now_unix_seconds.ok_or_else(|| {
        HttpApiError::bad_request(
            "missing_result_time",
            "result submission requires now_unix_seconds",
        )
    })?;
    let response = result_response("accepted", &request);
    let result = request.into_core()?;
    let mut control_plane = state.lock()?;
    control_plane
        .submit_result(&identity, result, now_unix_seconds)
        .map_err(runtime_error)?;
    Ok(Json(response))
}

async fn replay_spool(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(request): Json<SpoolReplayRequest>,
) -> Result<Json<SpoolReplayResponse>, HttpApiError> {
    let identity = identity_from_headers(&headers)?;
    let _replay_reason = request.reason.as_deref();
    let result = request.result.into_core()?;
    let mut control_plane = state.lock()?;
    control_plane
        .submit_result(&identity, result, request.now_unix_seconds)
        .map_err(runtime_error)?;
    Ok(Json(SpoolReplayResponse {
        status: "accepted",
        spool_id: request.spool_id,
    }))
}

impl HttpState {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ControlPlane>, HttpApiError> {
        self.control_plane.lock().map_err(|_| {
            HttpApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "control_plane_lock_poisoned",
                "control plane state lock is poisoned",
            )
        })
    }
}

impl AgentResultSubmissionDto {
    fn into_core(self) -> Result<AgentResultSubmission, HttpApiError> {
        let status = parse_result_status(&self.status)?;
        Ok(AgentResultSubmission::new(
            self.envelope_id,
            self.run_id,
            self.task_id,
            self.node_id,
            self.nonce,
            status,
            self.evidence.into_iter().map(EvidenceEnvelope::from),
            self.audit_correlation_id,
        ))
    }
}

impl From<EvidenceDto> for EvidenceEnvelope {
    fn from(value: EvidenceDto) -> Self {
        Self {
            source: value.source,
            content_type: value.content_type,
            body: value.body,
            truncated: value.truncated,
        }
    }
}

impl From<runlane_core::AgentTaskEnvelope> for AgentTaskEnvelopeDto {
    fn from(value: runlane_core::AgentTaskEnvelope) -> Self {
        Self {
            envelope_id: value.envelope_id,
            run_id: value.run_id,
            task_id: value.task_id,
            node_id: value.node_id,
            issued_at_unix_seconds: value.issued_at_unix_seconds,
            expires_at_unix_seconds: value.expires_at_unix_seconds,
            nonce: value.nonce,
            required_capabilities: value
                .required_capabilities
                .into_iter()
                .map(|capability| capability.as_str().to_owned())
                .collect(),
            audit_correlation_id: value.audit_correlation_id,
        }
    }
}

impl From<runlane_core::runtime::TypedTaskPayload> for TypedTaskPayloadDto {
    fn from(value: runlane_core::runtime::TypedTaskPayload) -> Self {
        match value {
            runlane_core::runtime::TypedTaskPayload::CollectEvidence {
                capability,
                resource_id,
            } => Self::CollectEvidence {
                capability: capability.as_str().to_owned(),
                resource_id,
            },
            runlane_core::runtime::TypedTaskPayload::ExecuteTypedHelperRequest {
                action,
                target_resource_id,
            } => Self::ExecuteTypedHelperRequest {
                action,
                target_resource_id,
            },
            runlane_core::runtime::TypedTaskPayload::Verify {
                check_id,
                resource_id,
            } => Self::Verify {
                check_id,
                resource_id,
            },
        }
    }
}

fn identity_from_headers(headers: &HeaderMap) -> Result<AgentIdentity, HttpApiError> {
    let node_id = required_header(headers, HEADER_NODE_ID)?;
    let certificate_fingerprint = required_header(headers, HEADER_CERT_FINGERPRINT)?;
    Ok(AgentIdentity::new(node_id, certificate_fingerprint))
}

fn required_header(headers: &HeaderMap, name: &'static str) -> Result<String, HttpApiError> {
    headers
        .get(name)
        .ok_or_else(|| {
            HttpApiError::unauthorized(
                "missing_identity",
                format!("missing required identity header {name}"),
            )
        })?
        .to_str()
        .map_err(|_| {
            HttpApiError::bad_request(
                "invalid_identity_header",
                format!("identity header {name} is not valid UTF-8"),
            )
        })
        .map(str::to_owned)
}

fn parse_operating_system(value: &str) -> Result<OperatingSystem, HttpApiError> {
    match value {
        "linux" => Ok(OperatingSystem::Linux),
        "freebsd" => Ok(OperatingSystem::FreeBsd),
        "openbsd" => Ok(OperatingSystem::OpenBsd),
        _ => Err(HttpApiError::bad_request(
            "invalid_platform_family",
            format!("unsupported platform_family {value:?}"),
        )),
    }
}

fn format_operating_system(value: OperatingSystem) -> &'static str {
    match value {
        OperatingSystem::Linux => "linux",
        OperatingSystem::FreeBsd => "freebsd",
        OperatingSystem::OpenBsd => "openbsd",
        OperatingSystem::Solaris => "solaris",
        OperatingSystem::Illumos => "illumos",
        OperatingSystem::Unknown => "unknown",
    }
}

fn parse_result_status(value: &str) -> Result<AgentResultStatus, HttpApiError> {
    match value {
        "succeeded" => Ok(AgentResultStatus::Succeeded),
        "failed" => Ok(AgentResultStatus::Failed),
        _ => Err(HttpApiError::bad_request(
            "invalid_result_status",
            format!("unsupported result status {value:?}"),
        )),
    }
}

fn result_response(
    status: &'static str,
    request: &AgentResultSubmissionDto,
) -> AgentResultResponse {
    AgentResultResponse {
        status,
        envelope_id: request.envelope_id.clone(),
        task_id: request.task_id.clone(),
        node_id: request.node_id.clone(),
    }
}

fn runtime_error(error: RuntimeError) -> HttpApiError {
    match error {
        RuntimeError::UnknownAgent => {
            HttpApiError::unauthorized("unknown_agent", "agent identity is not enrolled")
        }
        RuntimeError::AgentIdentityMismatch => HttpApiError::new(
            StatusCode::FORBIDDEN,
            "agent_identity_mismatch",
            "certificate fingerprint does not match enrolled agent identity",
        ),
        RuntimeError::NoTaskAvailable => HttpApiError::new(
            StatusCode::NOT_FOUND,
            "no_task_available",
            "no task available",
        ),
        RuntimeError::AuditAppend(error) => HttpApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "audit_append_failed",
            format!("{error:?}"),
        ),
        other => HttpApiError::conflict("control_plane_rejected", format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{HEADER_CERT_FINGERPRINT, HEADER_NODE_ID, HttpState, router};
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use runlane_core::{
        AgentTaskEnvelope, Capability, OperatingSystem,
        runtime::{
            AgentEnrollmentRequest, ControlPlane, EnrollmentToken, PendingAgentTask,
            TypedTaskPayload,
        },
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn enrollment_pull_result_and_spool_replay_use_core_boundary() {
        let app = router(HttpState::new(seeded_control_plane(200)));

        let pull_response = request(
            app.clone(),
            "/v1/agent/pull",
            json!({
                "node_id": "prod-web-01",
                "now_unix_seconds": 101,
                "capability_report_version": "cap-1",
                "last_seen_task_nonce": null
            }),
            true,
        )
        .await;
        assert_eq!(pull_response.status, StatusCode::OK);
        assert_eq!(pull_response.body["envelope"]["task_id"], "task-1");
        assert_eq!(pull_response.body["payload"]["type"], "collect_evidence");

        let result = json!({
            "envelope_id": "env-1",
            "run_id": "run-1",
            "task_id": "task-1",
            "node_id": "prod-web-01",
            "nonce": "nonce-1",
            "status": "succeeded",
            "now_unix_seconds": 102,
            "evidence": [
                {
                    "source": "service_status",
                    "content_type": "text/plain",
                    "body": "sshd active",
                    "truncated": false
                }
            ],
            "audit_correlation_id": "audit-1"
        });
        let submit_response = request(app.clone(), "/v1/agent/result", result.clone(), true).await;
        assert_eq!(submit_response.status, StatusCode::OK);
        assert_eq!(submit_response.body["status"], "accepted");

        let second_pull = request(
            app.clone(),
            "/v1/agent/pull",
            json!({
                "node_id": "prod-web-01",
                "now_unix_seconds": 101
            }),
            true,
        )
        .await;
        assert_eq!(second_pull.status, StatusCode::OK);
        assert_eq!(second_pull.body["envelope"]["task_id"], "task-2");

        let replay_response = request(
            app,
            "/v1/agent/spool/replay",
            json!({
                "spool_id": "spool-1",
                "reason": "server unavailable",
                "result": {
                    "envelope_id": "env-2",
                    "run_id": "run-1",
                    "task_id": "task-2",
                    "node_id": "prod-web-01",
                    "nonce": "nonce-2",
                    "status": "succeeded",
                    "evidence": [
                        {
                            "source": "service_status",
                            "content_type": "text/plain",
                            "body": "sshd active",
                            "truncated": false
                        }
                    ],
                    "audit_correlation_id": "audit-2"
                },
                "now_unix_seconds": 102
            }),
            true,
        )
        .await;
        assert_eq!(replay_response.status, StatusCode::OK);
        assert_eq!(replay_response.body["spool_id"], "spool-1");
    }

    #[tokio::test]
    async fn missing_identity_fails_closed_before_pull() {
        let app = router(HttpState::new(seeded_control_plane(200)));
        let response = request(
            app,
            "/v1/agent/pull",
            json!({
                "node_id": "prod-web-01",
                "now_unix_seconds": 101
            }),
            false,
        )
        .await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert_eq!(response.body["kind"], "missing_identity");
    }

    #[tokio::test]
    async fn wrong_identity_expired_envelope_replay_and_malformed_result_fail_closed() {
        let wrong_identity = request(
            router(HttpState::new(seeded_control_plane(200))),
            "/v1/agent/pull",
            json!({
                "node_id": "prod-web-01",
                "now_unix_seconds": 101
            }),
            true_with_cert("wrong-fingerprint"),
        )
        .await;
        assert_eq!(wrong_identity.status, StatusCode::FORBIDDEN);
        assert_eq!(wrong_identity.body["kind"], "agent_identity_mismatch");

        let expired = request(
            router(HttpState::new(seeded_control_plane(100))),
            "/v1/agent/pull",
            json!({
                "node_id": "prod-web-01",
                "now_unix_seconds": 101
            }),
            true,
        )
        .await;
        assert_eq!(expired.status, StatusCode::CONFLICT);

        let app = router(HttpState::new(seeded_control_plane(200)));
        let _ = request(
            app.clone(),
            "/v1/agent/pull",
            json!({
                "node_id": "prod-web-01",
                "now_unix_seconds": 101
            }),
            true,
        )
        .await;
        let result = json!({
            "envelope_id": "env-1",
            "run_id": "run-1",
            "task_id": "task-1",
            "node_id": "prod-web-01",
            "nonce": "nonce-1",
            "status": "succeeded",
            "now_unix_seconds": 102,
            "evidence": [],
            "audit_correlation_id": "audit-1"
        });
        let first = request(app.clone(), "/v1/agent/result", result.clone(), true).await;
        assert_eq!(first.status, StatusCode::OK);
        let replay = request(app.clone(), "/v1/agent/result", result, true).await;
        assert_eq!(replay.status, StatusCode::CONFLICT);

        let malformed = request(
            app,
            "/v1/agent/result",
            json!({
                "envelope_id": "env-2",
                "run_id": "run-1",
                "task_id": "task-2",
                "node_id": "prod-web-01",
                "nonce": "nonce-2",
                "status": "maybe",
                "now_unix_seconds": 102,
                "evidence": [],
                "audit_correlation_id": "audit-2"
            }),
            true,
        )
        .await;
        assert_eq!(malformed.status, StatusCode::BAD_REQUEST);
        assert_eq!(malformed.body["kind"], "invalid_result_status");
    }

    #[tokio::test]
    async fn expired_enrollment_token_fails_closed() {
        let app = router(HttpState::new(ControlPlane::empty()));
        let created = request(
            app.clone(),
            "/v1/enrollment/tokens",
            json!({
                "token_id": "token-1",
                "token": "secret",
                "node_id": "prod-web-01",
                "platform_family": "linux",
                "server_trust_root": "trust-root",
                "expires_at_unix_seconds": 100,
                "nonce": "enroll-nonce"
            }),
            false,
        )
        .await;
        assert_eq!(created.status, StatusCode::OK);

        let expired = request(
            app,
            "/v1/agent/enroll",
            json!({
                "token": "secret",
                "node_id": "prod-web-01",
                "platform_family": "linux",
                "certificate_fingerprint": "cert-fingerprint",
                "server_trust_root": "trust-root",
                "now_unix_seconds": 101
            }),
            false,
        )
        .await;
        assert_eq!(expired.status, StatusCode::CONFLICT);
        assert_eq!(expired.body["kind"], "control_plane_rejected");
    }

    struct TestResponse {
        status: StatusCode,
        body: Value,
    }

    async fn request(
        app: axum::Router,
        path: &str,
        body: Value,
        identity: impl IntoIdentityHeaders,
    ) -> TestResponse {
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json");
        identity.apply(&mut builder);
        let request = builder
            .body(Body::from(serde_json::to_vec(&body).expect("json body")))
            .expect("request builds");
        let response = app.oneshot(request).await.expect("router responds");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let body = serde_json::from_slice::<Value>(&bytes).unwrap_or_else(|_| json!({}));
        TestResponse { status, body }
    }

    trait IntoIdentityHeaders {
        fn apply(self, builder: &mut axum::http::request::Builder);
    }

    impl IntoIdentityHeaders for bool {
        fn apply(self, builder: &mut axum::http::request::Builder) {
            if self {
                builder
                    .headers_mut()
                    .expect("headers")
                    .insert(HEADER_NODE_ID, "prod-web-01".parse().expect("node header"));
                builder.headers_mut().expect("headers").insert(
                    HEADER_CERT_FINGERPRINT,
                    "cert-fingerprint".parse().expect("cert header"),
                );
            }
        }
    }

    struct CertOverride(&'static str);

    fn true_with_cert(cert: &'static str) -> CertOverride {
        CertOverride(cert)
    }

    impl IntoIdentityHeaders for CertOverride {
        fn apply(self, builder: &mut axum::http::request::Builder) {
            builder
                .headers_mut()
                .expect("headers")
                .insert(HEADER_NODE_ID, "prod-web-01".parse().expect("node header"));
            builder.headers_mut().expect("headers").insert(
                HEADER_CERT_FINGERPRINT,
                self.0.parse().expect("cert header"),
            );
        }
    }

    fn seeded_control_plane(task_expires_at: u64) -> ControlPlane {
        let mut control_plane = ControlPlane::empty();
        control_plane
            .create_enrollment_token(EnrollmentToken::new(
                "token-prod-web-01",
                "demo-token",
                "prod-web-01",
                OperatingSystem::Linux,
                "trust-root",
                200,
                "enroll-nonce",
            ))
            .expect("token valid");
        control_plane
            .enroll_agent(&AgentEnrollmentRequest::new(
                "demo-token",
                "prod-web-01",
                OperatingSystem::Linux,
                "cert-fingerprint",
                "trust-root",
                100,
            ))
            .expect("agent enrolled");
        control_plane.enqueue_task(PendingAgentTask::new(
            AgentTaskEnvelope::new(
                "env-1",
                "run-1",
                "task-1",
                "prod-web-01",
                100,
                task_expires_at,
                "nonce-1",
                [Capability::new("service.systemd")],
                "audit-1",
            ),
            TypedTaskPayload::CollectEvidence {
                capability: Capability::new("service.systemd"),
                resource_id: "system:node/prod-web-01/service/sshd".to_owned(),
            },
        ));
        control_plane.enqueue_task(PendingAgentTask::new(
            AgentTaskEnvelope::new(
                "env-2",
                "run-1",
                "task-2",
                "prod-web-01",
                100,
                task_expires_at,
                "nonce-2",
                [Capability::new("service.systemd")],
                "audit-2",
            ),
            TypedTaskPayload::CollectEvidence {
                capability: Capability::new("service.systemd"),
                resource_id: "system:node/prod-web-01/service/sshd".to_owned(),
            },
        ));
        control_plane
    }
}
