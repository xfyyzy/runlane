use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    AuditAppendError, AuditEvent, AuditLedger,
    receipt::{OperatorReceipt, ReceiptError, generate_operator_receipt},
};

/// Local server state directory layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStateLayout {
    pub root: PathBuf,
    pub ledger_dir: PathBuf,
    pub audit_ledger: PathBuf,
}

impl ServerStateLayout {
    /// Creates paths for a local server state root.
    #[must_use]
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let ledger_dir = root.join("ledger");
        let audit_ledger = ledger_dir.join("audit.yaml");
        Self {
            root,
            ledger_dir,
            audit_ledger,
        }
    }
}

/// Durable local state store backed by append-only audit event documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalServerState {
    pub layout: ServerStateLayout,
}

impl LocalServerState {
    /// Initializes a local server state directory without writing runtime truth to fleet Git.
    pub fn init(root: impl AsRef<Path>) -> Result<Self, DurableStateError> {
        let layout = ServerStateLayout::new(root);
        fs::create_dir_all(&layout.ledger_dir).map_err(|error| DurableStateError::Io {
            path: layout.ledger_dir.clone(),
            operation: "create ledger directory",
            message: error.to_string(),
        })?;
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&layout.audit_ledger)
            .or_else(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    Ok(OpenOptions::new().append(true).open(&layout.audit_ledger)?)
                } else {
                    Err(error)
                }
            })
            .map_err(|error| DurableStateError::Io {
                path: layout.audit_ledger.clone(),
                operation: "create audit ledger",
                message: error.to_string(),
            })?;
        Ok(Self { layout })
    }

    /// Opens an existing local server state directory.
    #[must_use]
    pub fn open(root: impl AsRef<Path>) -> Self {
        Self {
            layout: ServerStateLayout::new(root),
        }
    }

    /// Appends one event after validating the durable sequence remains monotonic.
    pub fn append_event(&self, event: &AuditEvent) -> Result<(), DurableStateError> {
        let ledger = self.load_ledger()?;
        let expected = ledger.next_sequence();
        if event.sequence != expected {
            return Err(DurableStateError::Append(
                AuditAppendError::NonMonotonicSequence {
                    expected,
                    actual: event.sequence,
                },
            ));
        }

        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.layout.audit_ledger)
            .map_err(|error| DurableStateError::Io {
                path: self.layout.audit_ledger.clone(),
                operation: "open audit ledger for append",
                message: error.to_string(),
            })?;
        writeln!(file, "---").map_err(|error| DurableStateError::Io {
            path: self.layout.audit_ledger.clone(),
            operation: "write audit document separator",
            message: error.to_string(),
        })?;
        serde_yaml::to_writer(&mut file, event).map_err(|error| {
            DurableStateError::CorruptWrite {
                path: self.layout.audit_ledger.clone(),
                message: error.to_string(),
            }
        })?;
        writeln!(file).map_err(|error| DurableStateError::Io {
            path: self.layout.audit_ledger.clone(),
            operation: "finish audit document",
            message: error.to_string(),
        })?;
        Ok(())
    }

    /// Appends every event from an in-memory ledger.
    pub fn append_ledger(&self, ledger: &AuditLedger) -> Result<(), DurableStateError> {
        for event in ledger.events() {
            self.append_event(event)?;
        }
        Ok(())
    }

    /// Loads the append-only audit ledger and validates event sequencing.
    pub fn load_ledger(&self) -> Result<AuditLedger, DurableStateError> {
        if !self.layout.audit_ledger.exists() {
            return Err(DurableStateError::MissingLedger {
                path: self.layout.audit_ledger.clone(),
            });
        }
        let content = fs::read_to_string(&self.layout.audit_ledger).map_err(|error| {
            DurableStateError::Io {
                path: self.layout.audit_ledger.clone(),
                operation: "read audit ledger",
                message: error.to_string(),
            }
        })?;
        if content.trim().is_empty() {
            return Ok(AuditLedger::empty());
        }
        let documents = serde_yaml::Deserializer::from_str(&content);
        let mut ledger = AuditLedger::empty();
        for document in documents {
            let event = AuditEvent::deserialize(document).map_err(|error| {
                DurableStateError::CorruptLedger {
                    path: self.layout.audit_ledger.clone(),
                    message: error.to_string(),
                }
            })?;
            ledger.append(event).map_err(DurableStateError::Append)?;
        }
        Ok(ledger)
    }

    /// Renders a receipt from the durable ledger after a process restart.
    pub fn render_receipt(&self, run_id: &str) -> Result<OperatorReceipt, DurableStateError> {
        let ledger = self.load_ledger()?;
        generate_operator_receipt(run_id, &ledger).map_err(DurableStateError::Receipt)
    }
}

/// Durable state failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurableStateError {
    MissingLedger {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        operation: &'static str,
        message: String,
    },
    CorruptLedger {
        path: PathBuf,
        message: String,
    },
    CorruptWrite {
        path: PathBuf,
        message: String,
    },
    Append(AuditAppendError),
    Receipt(ReceiptError),
}

impl std::fmt::Display for DurableStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingLedger { path } => {
                write!(formatter, "missing audit ledger: {}", path.display())
            }
            Self::Io {
                path,
                operation,
                message,
            } => write!(
                formatter,
                "failed to {operation} at {}: {message}",
                path.display()
            ),
            Self::CorruptLedger { path, message } => {
                write!(
                    formatter,
                    "corrupt audit ledger {}: {message}",
                    path.display()
                )
            }
            Self::CorruptWrite { path, message } => {
                write!(
                    formatter,
                    "failed to serialize audit event to {}: {message}",
                    path.display()
                )
            }
            Self::Append(error) => write!(formatter, "append-only ledger violation: {error:?}"),
            Self::Receipt(error) => write!(formatter, "receipt generation failed: {error:?}"),
        }
    }
}

impl std::error::Error for DurableStateError {}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        AuditAppendError, AuditEvent, AuditEventKind, AuditLedger,
        durable::{DurableStateError, LocalServerState},
        e2e::run_service_unhealthy_simulation,
    };

    #[test]
    fn appends_and_reloads_demo_ledger() {
        let state = LocalServerState::init(test_state_dir("reload")).expect("state initializes");
        let simulation = demo_simulation();
        state
            .append_ledger(&simulation.ledger)
            .expect("demo ledger persists");

        let reloaded = state.load_ledger().expect("ledger reloads");
        assert_eq!(reloaded, simulation.ledger);
        assert!(
            reloaded
                .events()
                .iter()
                .any(|event| { matches!(event.kind, AuditEventKind::AgentEnrolled { .. }) })
        );
        assert!(
            reloaded
                .events()
                .iter()
                .any(|event| { matches!(event.kind, AuditEventKind::AgentResultAccepted { .. }) })
        );
        assert!(reloaded.events().iter().any(|event| {
            matches!(event.kind, AuditEventKind::CognitiveReceiptGenerated { .. })
        }));
    }

    #[test]
    fn rejects_non_monotonic_append() {
        let state = LocalServerState::init(test_state_dir("append")).expect("state initializes");
        state
            .append_event(&AuditEvent::new(
                "event-1",
                "run-1",
                1,
                AuditEventKind::EvidenceCollected {
                    source: "service_status".to_owned(),
                },
            ))
            .expect("first event appends");

        assert!(matches!(
            state.append_event(&AuditEvent::new(
                "event-2",
                "run-1",
                1,
                AuditEventKind::EvidenceCollected {
                    source: "recent_logs".to_owned(),
                },
            )),
            Err(DurableStateError::Append(
                AuditAppendError::NonMonotonicSequence {
                    expected: 2,
                    actual: 1
                }
            ))
        ));
    }

    #[test]
    fn missing_and_corrupt_state_fail_explicitly() {
        let missing = LocalServerState::open(test_state_dir("missing"));
        assert!(matches!(
            missing.load_ledger(),
            Err(DurableStateError::MissingLedger { .. })
        ));

        let state = LocalServerState::init(test_state_dir("corrupt")).expect("state initializes");
        fs::write(&state.layout.audit_ledger, "not: [valid").expect("corrupt fixture writes");
        assert!(matches!(
            state.load_ledger(),
            Err(DurableStateError::CorruptLedger { .. })
        ));
    }

    #[test]
    fn renders_receipt_after_reload() {
        let state = LocalServerState::init(test_state_dir("receipt")).expect("state initializes");
        let simulation = demo_simulation();
        state
            .append_ledger(&simulation.ledger)
            .expect("demo ledger persists");

        let receipt = LocalServerState::open(&state.layout.root)
            .render_receipt(&simulation.run_id)
            .expect("receipt renders after restart");
        assert_eq!(receipt.render_text(), simulation.receipt.render_text());
    }

    #[test]
    fn incomplete_durable_ledger_preserves_receipt_failure() {
        let state =
            LocalServerState::init(test_state_dir("incomplete")).expect("state initializes");
        let mut ledger = AuditLedger::empty();
        ledger
            .append(AuditEvent::new(
                "event-1",
                "run-1",
                1,
                AuditEventKind::IncidentCreated {
                    incident_id: "incident-1".to_owned(),
                    node_id: "prod-web-01".to_owned(),
                    runbook: "service-unhealthy".to_owned(),
                },
            ))
            .expect("event appends");
        state.append_ledger(&ledger).expect("ledger persists");

        assert!(matches!(
            state.render_receipt("run-1"),
            Err(DurableStateError::Receipt(_))
        ));
    }

    fn demo_simulation() -> crate::e2e::ServiceUnhealthySimulation {
        run_service_unhealthy_simulation(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/fleet"
        ))
        .expect("demo simulation succeeds")
    }

    fn test_state_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "runlane-durable-{name}-{}-{nonce}",
            std::process::id()
        ))
    }
}
