use crate::{
    ActionKind, CapabilityLeaseClaims, VerificationPlan,
    approval::{ApprovalError, ApprovalRecord, ApprovalStore},
};

/// Telegram user identity presented to the approval adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramIdentity {
    pub chat_id: i64,
    pub user_id: i64,
    pub username: Option<String>,
}

impl TelegramIdentity {
    /// Creates a Telegram identity with explicit chat and user ids.
    #[must_use]
    pub fn new(chat_id: i64, user_id: i64, username: Option<String>) -> Self {
        Self {
            chat_id,
            user_id,
            username,
        }
    }
}

/// Authorized mapping from Telegram identity to Runlane audit actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramAuthorizedActor {
    pub chat_id: i64,
    pub user_id: i64,
    pub actor: String,
}

impl TelegramAuthorizedActor {
    /// Creates an authorized Telegram-to-Runlane actor mapping.
    #[must_use]
    pub fn new(chat_id: i64, user_id: i64, actor: impl Into<String>) -> Self {
        Self {
            chat_id,
            user_id,
            actor: actor.into(),
        }
    }
}

/// Explicit Telegram identity allowlist for approval decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramIdentityMap {
    actors: Vec<TelegramAuthorizedActor>,
}

impl TelegramIdentityMap {
    /// Creates an identity map from authorized actors.
    #[must_use]
    pub fn new(actors: impl IntoIterator<Item = TelegramAuthorizedActor>) -> Self {
        Self {
            actors: actors.into_iter().collect(),
        }
    }

    fn actor_for(&self, identity: &TelegramIdentity) -> Result<&str, TelegramApprovalError> {
        self.actors
            .iter()
            .find(|actor| actor.chat_id == identity.chat_id && actor.user_id == identity.user_id)
            .map(|actor| actor.actor.as_str())
            .ok_or(TelegramApprovalError::UnauthorizedIdentity {
                chat_id: identity.chat_id,
                user_id: identity.user_id,
            })
    }
}

/// CI-safe Telegram approval command shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramApprovalCommand {
    List,
    Show { approval_id: String },
    Approve { approval_id: String },
    Reject { approval_id: String },
}

impl TelegramApprovalCommand {
    /// Parses a Telegram message into the narrow approval command enum.
    pub fn parse(message: &str) -> Result<Self, TelegramApprovalError> {
        let parts = message.split_whitespace().collect::<Vec<_>>();
        match parts.as_slice() {
            ["/approvals"] | ["/approval", "list"] => Ok(Self::List),
            ["/approval", "show", approval_id] | ["/show", approval_id] => Ok(Self::Show {
                approval_id: (*approval_id).to_owned(),
            }),
            ["/approval", "approve", approval_id] | ["/approve", approval_id] => {
                Ok(Self::Approve {
                    approval_id: (*approval_id).to_owned(),
                })
            }
            ["/approval", "reject", approval_id] | ["/reject", approval_id] => Ok(Self::Reject {
                approval_id: (*approval_id).to_owned(),
            }),
            [command, ..] => Err(TelegramApprovalError::UnsupportedCommand(
                (*command).to_owned(),
            )),
            [] => Err(TelegramApprovalError::UnsupportedCommand(String::new())),
        }
    }
}

/// Runtime context supplied by the server-side adapter boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramApprovalContext {
    pub now_unix_seconds: u64,
    pub allowlist_entry_id: String,
    pub lease_nonce: String,
}

impl TelegramApprovalContext {
    /// Creates approval execution context for approve/reject operations.
    #[must_use]
    pub fn new(
        now_unix_seconds: u64,
        allowlist_entry_id: impl Into<String>,
        lease_nonce: impl Into<String>,
    ) -> Self {
        Self {
            now_unix_seconds,
            allowlist_entry_id: allowlist_entry_id.into(),
            lease_nonce: lease_nonce.into(),
        }
    }
}

/// Summary rendered to a Telegram approval list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramApprovalSummary {
    pub id: String,
    pub action_id: String,
    pub target_resource_id: String,
    pub expires_at_unix_seconds: u64,
}

impl TelegramApprovalSummary {
    fn from_record(record: &ApprovalRecord) -> Self {
        Self {
            id: record.id.clone(),
            action_id: record.action_id.clone(),
            target_resource_id: record.target.resource_id.clone(),
            expires_at_unix_seconds: record.expires_at_unix_seconds,
        }
    }
}

/// Detail rendered to a Telegram approval show view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramApprovalDetail {
    pub summary: TelegramApprovalSummary,
    pub required_checks: usize,
    pub skipped_checks: usize,
    pub verification: VerificationPlan,
}

impl TelegramApprovalDetail {
    fn from_record(record: &ApprovalRecord) -> Self {
        Self {
            summary: TelegramApprovalSummary::from_record(record),
            required_checks: record.verification.required.len(),
            skipped_checks: record.verification.skipped.len(),
            verification: record.verification.clone(),
        }
    }
}

/// Telegram adapter response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramApprovalResponse {
    PendingApprovals(Vec<TelegramApprovalSummary>),
    ApprovalDetail(TelegramApprovalDetail),
    Approved {
        approval_id: String,
        lease_id: String,
        action: ActionKind,
        actor: String,
    },
    Rejected {
        approval_id: String,
        actor: String,
    },
}

/// Telegram adapter failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramApprovalError {
    UnauthorizedIdentity { chat_id: i64, user_id: i64 },
    UnsupportedCommand(String),
    UnknownApproval(String),
    Approval(ApprovalError),
}

impl From<ApprovalError> for TelegramApprovalError {
    fn from(error: ApprovalError) -> Self {
        Self::Approval(error)
    }
}

/// Telegram approval adapter.
///
/// This adapter owns no operation logic. It only maps Telegram identity to a
/// Runlane actor, parses narrow approval commands, and calls [`ApprovalStore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramApprovalAdapter {
    identity_map: TelegramIdentityMap,
}

impl TelegramApprovalAdapter {
    /// Creates an adapter with explicit authorized Telegram actors.
    #[must_use]
    pub fn new(identity_map: TelegramIdentityMap) -> Self {
        Self { identity_map }
    }

    /// Handles one Telegram approval command through the shared approval API.
    pub fn handle_command(
        &self,
        store: &mut ApprovalStore,
        identity: &TelegramIdentity,
        command: TelegramApprovalCommand,
        context: &TelegramApprovalContext,
    ) -> Result<TelegramApprovalResponse, TelegramApprovalError> {
        let actor = match self.identity_map.actor_for(identity) {
            Ok(actor) => actor.to_owned(),
            Err(error) => {
                store.record_adapter_rejection("telegram", "unauthorized_identity")?;
                return Err(error);
            }
        };
        match command {
            TelegramApprovalCommand::List => Ok(TelegramApprovalResponse::PendingApprovals(
                store
                    .list_pending()
                    .into_iter()
                    .map(TelegramApprovalSummary::from_record)
                    .collect(),
            )),
            TelegramApprovalCommand::Show { approval_id } => {
                let record = store
                    .show(&approval_id)
                    .ok_or_else(|| TelegramApprovalError::UnknownApproval(approval_id.clone()))?;
                Ok(TelegramApprovalResponse::ApprovalDetail(
                    TelegramApprovalDetail::from_record(record),
                ))
            }
            TelegramApprovalCommand::Approve { approval_id } => {
                let action_id = store
                    .show(&approval_id)
                    .ok_or_else(|| TelegramApprovalError::UnknownApproval(approval_id.clone()))?
                    .action_id
                    .clone();
                let claims: CapabilityLeaseClaims = store.approve(
                    &approval_id,
                    &action_id,
                    &actor,
                    context.now_unix_seconds,
                    &context.allowlist_entry_id,
                    &context.lease_nonce,
                )?;
                Ok(TelegramApprovalResponse::Approved {
                    approval_id,
                    lease_id: claims.lease_id,
                    action: claims.action,
                    actor,
                })
            }
            TelegramApprovalCommand::Reject { approval_id } => {
                store.reject(&approval_id, &actor, context.now_unix_seconds)?;
                Ok(TelegramApprovalResponse::Rejected { approval_id, actor })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ActionKind, ApprovalOutcome, AuditEventKind,
        approval::{ApprovalState, demo_approval_store},
    };

    use super::{
        TelegramApprovalAdapter, TelegramApprovalCommand, TelegramApprovalContext,
        TelegramApprovalError, TelegramApprovalResponse, TelegramAuthorizedActor, TelegramIdentity,
        TelegramIdentityMap,
    };

    fn adapter() -> TelegramApprovalAdapter {
        TelegramApprovalAdapter::new(TelegramIdentityMap::new([TelegramAuthorizedActor::new(
            42,
            1001,
            "telegram:alice",
        )]))
    }

    fn identity() -> TelegramIdentity {
        TelegramIdentity::new(42, 1001, Some("alice".to_owned()))
    }

    fn context() -> TelegramApprovalContext {
        TelegramApprovalContext::new(150, "allow-prod-web-sshd-restart", "telegram-lease-nonce")
    }

    #[test]
    fn telegram_adapter_lists_shows_and_approves_via_approval_store() {
        let mut store = demo_approval_store();

        let listed = adapter()
            .handle_command(
                &mut store,
                &identity(),
                TelegramApprovalCommand::parse("/approvals").expect("list command parses"),
                &context(),
            )
            .expect("authorized list succeeds");
        assert_eq!(
            listed,
            TelegramApprovalResponse::PendingApprovals(vec![super::TelegramApprovalSummary {
                id: "approval-demo-1".to_owned(),
                action_id: "restart-service".to_owned(),
                target_resource_id: "system:node/prod-web-01/service/sshd".to_owned(),
                expires_at_unix_seconds: 200,
            },])
        );

        let shown = adapter()
            .handle_command(
                &mut store,
                &identity(),
                TelegramApprovalCommand::parse("/approval show approval-demo-1")
                    .expect("show command parses"),
                &context(),
            )
            .expect("authorized show succeeds");
        match shown {
            TelegramApprovalResponse::ApprovalDetail(detail) => {
                assert_eq!(detail.summary.id, "approval-demo-1");
                assert_eq!(detail.required_checks, 1);
                assert_eq!(detail.skipped_checks, 2);
                assert!(detail.verification.skipped_checks_have_reasons());
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let approved = adapter()
            .handle_command(
                &mut store,
                &identity(),
                TelegramApprovalCommand::parse("/approve approval-demo-1")
                    .expect("approve command parses"),
                &context(),
            )
            .expect("authorized approve succeeds");
        assert_eq!(
            approved,
            TelegramApprovalResponse::Approved {
                approval_id: "approval-demo-1".to_owned(),
                lease_id: "lease-approval-demo-1".to_owned(),
                action: ActionKind::ServiceRestart,
                actor: "telegram:alice".to_owned(),
            }
        );
        assert_eq!(
            store.show("approval-demo-1").unwrap().state,
            ApprovalState::Approved
        );
        assert!(store.ledger.events().iter().any(|event| matches!(
            &event.kind,
            AuditEventKind::ApprovalDecision {
                actor,
                outcome: ApprovalOutcome::Approved,
                ..
            } if actor == "telegram:alice"
        )));
    }

    #[test]
    fn telegram_adapter_rejects_through_same_audit_path() {
        let mut store = demo_approval_store();

        let rejected = adapter()
            .handle_command(
                &mut store,
                &identity(),
                TelegramApprovalCommand::parse("/reject approval-demo-1")
                    .expect("reject command parses"),
                &context(),
            )
            .expect("authorized reject succeeds");
        assert_eq!(
            rejected,
            TelegramApprovalResponse::Rejected {
                approval_id: "approval-demo-1".to_owned(),
                actor: "telegram:alice".to_owned(),
            }
        );
        assert_eq!(
            store.show("approval-demo-1").unwrap().state,
            ApprovalState::Rejected
        );
        assert!(store.ledger.events().iter().any(|event| matches!(
            &event.kind,
            AuditEventKind::ApprovalDecision {
                actor,
                outcome: ApprovalOutcome::Rejected,
                ..
            } if actor == "telegram:alice"
        )));
    }

    #[test]
    fn telegram_adapter_fails_closed_for_unknown_identity_and_commands() {
        let mut store = demo_approval_store();
        let unknown = TelegramIdentity::new(42, 9999, Some("mallory".to_owned()));

        let error = adapter()
            .handle_command(
                &mut store,
                &unknown,
                TelegramApprovalCommand::List,
                &context(),
            )
            .expect_err("unknown Telegram identity is rejected");
        assert_eq!(
            error,
            TelegramApprovalError::UnauthorizedIdentity {
                chat_id: 42,
                user_id: 9999,
            }
        );
        assert_eq!(
            store.show("approval-demo-1").unwrap().state,
            ApprovalState::Pending
        );
        assert!(store.ledger.events().iter().any(|event| matches!(
            &event.kind,
            AuditEventKind::ApprovalAdapterRejected { adapter, reason }
                if adapter == "telegram" && reason == "unauthorized_identity"
        )));

        assert_eq!(
            TelegramApprovalCommand::parse("/schedule reboot prod-web-01")
                .expect_err("non-approval command is unsupported"),
            TelegramApprovalError::UnsupportedCommand("/schedule".to_owned())
        );
    }
}
