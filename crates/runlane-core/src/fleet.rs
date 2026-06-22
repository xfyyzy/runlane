use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_yaml::Value;

use crate::{
    FleetIntentSetting, FleetOverlayFragment, FleetOverlayTier, OperatingSystem, OperationalLayer,
    ResolvedFleetIntent, resolve_fleet_overlays,
};

/// Server-ingestable desired intent loaded from a fleet repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRepository {
    pub root: PathBuf,
    pub inventory: Vec<FleetInventoryNode>,
    pub roles: Vec<FleetRole>,
    pub runbooks: Vec<FleetRunbook>,
    pub policies: Vec<FleetPolicy>,
    pub allowlists: Vec<FleetAllowlistEntryDoc>,
    pub overlays: Vec<FleetOverlayFragment>,
    pub resolved_overlay: ResolvedFleetIntent,
}

impl FleetRepository {
    /// Loads and validates a fleet repository from the documented layout.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, FleetLoadError> {
        let root = path.as_ref().to_path_buf();
        let inventory = load_documents(&root.join("inventory"), parse_inventory)?;
        let roles = load_documents(&root.join("roles"), parse_role)?;
        let runbooks = load_documents(&root.join("runbooks"), parse_runbook)?;
        let policies = load_documents(&root.join("policies"), parse_policy)?;
        let allowlists = load_documents(&root.join("allowlists"), parse_allowlist)?;
        let overlays = load_overlays(&root.join("overlays"))?;
        let resolved_overlay = resolve_fleet_overlays(overlays.clone());

        validate_repository(&inventory, &roles, &runbooks, &policies, &allowlists)?;

        Ok(Self {
            root,
            inventory,
            roles,
            runbooks,
            policies,
            allowlists,
            overlays,
            resolved_overlay,
        })
    }

    /// Returns a compact summary suitable for CLI output and sync logs.
    #[must_use]
    pub fn summary(&self) -> FleetSyncSummary {
        FleetSyncSummary {
            nodes: self.inventory.len(),
            roles: self.roles.len(),
            runbooks: self.runbooks.len(),
            policies: self.policies.len(),
            allowlists: self.allowlists.len(),
            overlays: self.overlays.len(),
            resolved_settings: self.resolved_overlay.settings.len(),
        }
    }
}

/// Compact sync summary for server ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetSyncSummary {
    pub nodes: usize,
    pub roles: usize,
    pub runbooks: usize,
    pub policies: usize,
    pub allowlists: usize,
    pub overlays: usize,
    pub resolved_settings: usize,
}

/// Inventory node loaded from desired fleet intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetInventoryNode {
    pub id: String,
    pub hostname: String,
    pub os: OperatingSystem,
    pub labels: BTreeMap<String, String>,
    pub layers: FleetLayers,
    pub requested_capabilities: Vec<String>,
    pub policy_profile: String,
}

/// Layer declaration shared by inventory and roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetLayers {
    pub primary: OperationalLayer,
    pub supports: Vec<OperationalLayer>,
}

/// Role desired intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRole {
    pub id: String,
    pub primary_layer: OperationalLayer,
    pub enabled_runbooks: Vec<String>,
    pub policy_profile: String,
    pub enabled_allowlists: Vec<String>,
}

/// Runbook desired intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRunbook {
    pub name: String,
    pub version: String,
    pub layer: OperationalLayer,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub conflicts: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub allowed_actions: Vec<String>,
}

/// Policy desired intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetPolicy {
    pub id: String,
    pub require_signed_lease: bool,
    pub reject_replay: bool,
}

/// Helper allowlist desired intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetAllowlistEntryDoc {
    pub id: String,
    pub action: String,
    pub target_resource_id: String,
}

/// Explicit fleet loading failure.
#[derive(Debug)]
pub enum FleetLoadError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Yaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    Invalid {
        path: PathBuf,
        message: String,
    },
    Validation {
        message: String,
    },
}

impl std::fmt::Display for FleetLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Yaml { path, source } => {
                write!(formatter, "{}: invalid yaml: {source}", path.display())
            }
            Self::Invalid { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::Validation { message } => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for FleetLoadError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventoryNode {
    id: String,
    hostname: String,
    os: String,
    #[serde(default)]
    labels: BTreeMap<String, String>,
    layers: RawLayers,
    capabilities: RawCapabilities,
    policy: RawNodePolicy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayers {
    primary: String,
    #[serde(default)]
    supports: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCapabilities {
    #[serde(default)]
    requested: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNodePolicy {
    profile: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRole {
    id: String,
    layers: RawRoleLayers,
    runbooks: RawEnabledList,
    policies: RawNodePolicy,
    allowlists: RawEnabledList,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoleLayers {
    primary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnabledList {
    #[serde(default)]
    enabled: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunbook {
    name: String,
    version: String,
    summary: Option<String>,
    layer: String,
    select: Option<Value>,
    #[serde(default)]
    parameters: BTreeMap<String, Value>,
    resources: RawRunbookResources,
    #[serde(default)]
    collect: Vec<RawCollectStep>,
    analyze: RawAnalyze,
    policy: Option<Value>,
    #[serde(default)]
    recover: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunbookResources {
    #[serde(default)]
    reads: Vec<String>,
    #[serde(default)]
    writes: Vec<String>,
    #[serde(default)]
    conflicts: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCollectStep {
    id: String,
    capability: Option<String>,
    #[serde(default)]
    capability_any: Vec<String>,
    #[serde(default)]
    with: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAnalyze {
    mode: String,
    #[serde(default)]
    allowed_actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicy {
    id: String,
    approval: Option<Value>,
    verification: Option<Value>,
    helper: RawHelperPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHelperPolicy {
    require_signed_lease: bool,
    reject_replay: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAllowlist {
    id: String,
    action: String,
    target_resource_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOverlay {
    tier: String,
    name: Option<String>,
    #[serde(default)]
    settings: BTreeMap<String, String>,
}

fn load_documents<T>(
    directory: &Path,
    parser: fn(&Path, &str) -> Result<T, FleetLoadError>,
) -> Result<Vec<T>, FleetLoadError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut paths = yaml_paths(directory)?;
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let body = fs::read_to_string(path).map_err(|source| FleetLoadError::Io {
                path: path.clone(),
                source,
            })?;
            reject_runtime_truth(path, &body)?;
            parser(path, &body)
        })
        .collect()
}

fn load_overlays(directory: &Path) -> Result<Vec<FleetOverlayFragment>, FleetLoadError> {
    let overlays = load_documents(directory, parse_overlay)?;
    Ok(overlays)
}

fn yaml_paths(directory: &Path) -> Result<Vec<PathBuf>, FleetLoadError> {
    let mut paths = Vec::new();
    collect_yaml_paths(directory, &mut paths)?;
    Ok(paths)
}

fn collect_yaml_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), FleetLoadError> {
    for entry in fs::read_dir(directory).map_err(|source| FleetLoadError::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let path = entry
            .map_err(|source| FleetLoadError::Io {
                path: directory.to_path_buf(),
                source,
            })?
            .path();
        if path.is_dir() {
            collect_yaml_paths(&path, paths)?;
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| matches!(extension, "yaml" | "yml"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn reject_runtime_truth(path: &Path, body: &str) -> Result<(), FleetLoadError> {
    let value = serde_yaml::from_str::<Value>(body).map_err(|source| FleetLoadError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    if let Some(key) = find_runtime_key(&value) {
        Err(FleetLoadError::Invalid {
            path: path.to_path_buf(),
            message: format!("runtime field `{key}` is not allowed in fleet intent"),
        })
    } else {
        Ok(())
    }
}

fn find_runtime_key(value: &Value) -> Option<String> {
    const FORBIDDEN: &[&str] = &[
        "evidence",
        "approvals",
        "helper_output",
        "audit_events",
        "audit",
        "cognitive_receipts",
        "receipt",
        "results",
        "runtime",
    ];

    match value {
        Value::Mapping(mapping) => mapping.iter().find_map(|(key, value)| {
            let key = key.as_str()?;
            if FORBIDDEN.contains(&key) {
                Some(key.to_owned())
            } else {
                find_runtime_key(value)
            }
        }),
        Value::Sequence(sequence) => sequence.iter().find_map(find_runtime_key),
        _ => None,
    }
}

fn parse_inventory(path: &Path, body: &str) -> Result<FleetInventoryNode, FleetLoadError> {
    let raw = parse_yaml::<RawInventoryNode>(path, body)?;
    let layers = parse_layers(path, raw.layers)?;
    validate_capabilities(path, &raw.capabilities.requested)?;
    Ok(FleetInventoryNode {
        id: raw.id,
        hostname: raw.hostname,
        os: parse_os(path, &raw.os)?,
        labels: raw.labels,
        layers,
        requested_capabilities: raw.capabilities.requested,
        policy_profile: raw.policy.profile,
    })
}

fn parse_role(path: &Path, body: &str) -> Result<FleetRole, FleetLoadError> {
    let raw = parse_yaml::<RawRole>(path, body)?;
    Ok(FleetRole {
        id: raw.id,
        primary_layer: parse_layer(path, &raw.layers.primary)?,
        enabled_runbooks: raw.runbooks.enabled,
        policy_profile: raw.policies.profile,
        enabled_allowlists: raw.allowlists.enabled,
    })
}

fn parse_runbook(path: &Path, body: &str) -> Result<FleetRunbook, FleetLoadError> {
    let raw = parse_yaml::<RawRunbook>(path, body)?;
    let _desired_metadata = (
        &raw.summary,
        &raw.select,
        &raw.parameters,
        &raw.policy,
        &raw.recover,
    );
    let mut capabilities = Vec::new();
    for step in &raw.collect {
        if let Some(capability) = &step.capability {
            capabilities.push(capability.clone());
        }
        capabilities.extend(step.capability_any.clone());
        if step.id.trim().is_empty() {
            return Err(invalid(path, "collect step id must not be empty"));
        }
        let _argument_count = step.with.len();
    }
    validate_capabilities(path, &capabilities)?;
    validate_resources(
        path,
        raw.resources
            .reads
            .iter()
            .chain(&raw.resources.writes)
            .chain(&raw.resources.conflicts),
    )?;
    if raw.analyze.mode != "structured_proposal" {
        return Err(invalid(path, "analyze.mode must be structured_proposal"));
    }
    validate_actions(path, &raw.analyze.allowed_actions)?;

    Ok(FleetRunbook {
        name: raw.name,
        version: raw.version,
        layer: parse_layer(path, &raw.layer)?,
        reads: raw.resources.reads,
        writes: raw.resources.writes,
        conflicts: raw.resources.conflicts,
        required_capabilities: capabilities,
        allowed_actions: raw.analyze.allowed_actions,
    })
}

fn parse_policy(path: &Path, body: &str) -> Result<FleetPolicy, FleetLoadError> {
    let raw = parse_yaml::<RawPolicy>(path, body)?;
    let _desired_policy_sections = (&raw.approval, &raw.verification);
    Ok(FleetPolicy {
        id: raw.id,
        require_signed_lease: raw.helper.require_signed_lease,
        reject_replay: raw.helper.reject_replay,
    })
}

fn parse_allowlist(path: &Path, body: &str) -> Result<FleetAllowlistEntryDoc, FleetLoadError> {
    let raw = parse_yaml::<RawAllowlist>(path, body)?;
    validate_actions(path, std::slice::from_ref(&raw.action))?;
    validate_resources(path, std::iter::once(&raw.target_resource_id))?;
    Ok(FleetAllowlistEntryDoc {
        id: raw.id,
        action: raw.action,
        target_resource_id: raw.target_resource_id,
    })
}

fn parse_overlay(path: &Path, body: &str) -> Result<FleetOverlayFragment, FleetLoadError> {
    let raw = parse_yaml::<RawOverlay>(path, body)?;
    let tier = match raw.tier.as_str() {
        "global" => FleetOverlayTier::Global,
        "os" => FleetOverlayTier::Os(parse_os(path, required_name(path, raw.name.as_deref())?)?),
        "layer" => FleetOverlayTier::Layer(parse_layer(
            path,
            required_name(path, raw.name.as_deref())?,
        )?),
        "role" => FleetOverlayTier::Role(required_name(path, raw.name.as_deref())?.to_owned()),
        "environment" => {
            FleetOverlayTier::Environment(required_name(path, raw.name.as_deref())?.to_owned())
        }
        "node" => FleetOverlayTier::Node(required_name(path, raw.name.as_deref())?.to_owned()),
        _ => {
            return Err(invalid(
                path,
                "overlay tier must be global/os/layer/role/environment/node",
            ));
        }
    };

    Ok(FleetOverlayFragment::new(
        tier,
        raw.settings
            .into_iter()
            .map(|(key, value)| FleetIntentSetting::new(key, value)),
    ))
}

fn parse_yaml<T: for<'de> Deserialize<'de>>(path: &Path, body: &str) -> Result<T, FleetLoadError> {
    serde_yaml::from_str(body).map_err(|source| FleetLoadError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

fn parse_layers(path: &Path, raw: RawLayers) -> Result<FleetLayers, FleetLoadError> {
    let primary = parse_layer(path, &raw.primary)?;
    let supports = raw
        .supports
        .iter()
        .map(|layer| parse_layer(path, layer))
        .collect::<Result<Vec<_>, _>>()?;
    if !supports.contains(&primary) {
        return Err(invalid(path, "layers.supports must include layers.primary"));
    }
    Ok(FleetLayers { primary, supports })
}

fn parse_layer(path: &Path, value: &str) -> Result<OperationalLayer, FleetLoadError> {
    match value {
        "system" => Ok(OperationalLayer::System),
        "platform" => Ok(OperationalLayer::Platform),
        "application" => Ok(OperationalLayer::Application),
        _ => Err(invalid(
            path,
            "layer must be one of system, platform, application",
        )),
    }
}

fn parse_os(path: &Path, value: &str) -> Result<OperatingSystem, FleetLoadError> {
    match value {
        "linux" => Ok(OperatingSystem::Linux),
        "freebsd" => Ok(OperatingSystem::FreeBsd),
        "openbsd" => Ok(OperatingSystem::OpenBsd),
        _ => Err(invalid(path, "os must be one of linux, freebsd, openbsd")),
    }
}

fn validate_repository(
    inventory: &[FleetInventoryNode],
    roles: &[FleetRole],
    runbooks: &[FleetRunbook],
    policies: &[FleetPolicy],
    allowlists: &[FleetAllowlistEntryDoc],
) -> Result<(), FleetLoadError> {
    if inventory.is_empty() {
        return Err(FleetLoadError::Validation {
            message: "fleet inventory must contain at least one node".to_owned(),
        });
    }

    let runbook_names = runbooks
        .iter()
        .map(|runbook| runbook.name.as_str())
        .collect::<BTreeSet<_>>();
    let policy_ids = policies
        .iter()
        .map(|policy| policy.id.as_str())
        .collect::<BTreeSet<_>>();
    let allowlist_ids = allowlists
        .iter()
        .map(|allowlist| allowlist.id.as_str())
        .collect::<BTreeSet<_>>();

    for role in roles {
        for runbook in &role.enabled_runbooks {
            require_known("role runbook", runbook, &runbook_names)?;
        }
        require_known("role policy", &role.policy_profile, &policy_ids)?;
        for allowlist in &role.enabled_allowlists {
            require_known("role allowlist", allowlist, &allowlist_ids)?;
        }
    }

    for node in inventory {
        require_known("node policy", &node.policy_profile, &policy_ids)?;
    }

    Ok(())
}

fn require_known(label: &str, value: &str, known: &BTreeSet<&str>) -> Result<(), FleetLoadError> {
    if known.contains(value) {
        Ok(())
    } else {
        Err(FleetLoadError::Validation {
            message: format!("unknown {label}: {value}"),
        })
    }
}

fn validate_resources<'a>(
    path: &Path,
    resources: impl IntoIterator<Item = &'a String>,
) -> Result<(), FleetLoadError> {
    for resource in resources {
        if !matches!(
            resource.split_once(':'),
            Some(("system" | "platform" | "application", rest)) if !rest.trim().is_empty()
        ) {
            return Err(invalid(
                path,
                "resource ids must start with system:, platform:, or application:",
            ));
        }
    }
    Ok(())
}

fn validate_capabilities(path: &Path, capabilities: &[String]) -> Result<(), FleetLoadError> {
    for capability in capabilities {
        if !known_capabilities().contains(capability.as_str()) {
            return Err(invalid(
                path,
                &format!("unknown required capability `{capability}`"),
            ));
        }
    }
    Ok(())
}

fn validate_actions(path: &Path, actions: &[String]) -> Result<(), FleetLoadError> {
    for action in actions {
        if !matches!(
            action.as_str(),
            "service.restart"
                | "service.reload"
                | "file.remove_from_allowlist"
                | "collect.more_logs"
        ) {
            return Err(invalid(path, &format!("unknown action `{action}`")));
        }
    }
    Ok(())
}

fn known_capabilities() -> BTreeSet<&'static str> {
    [
        "os.linux",
        "os.freebsd",
        "os.openbsd",
        "service.systemd",
        "service.freebsd-rc",
        "service.openbsd-rcctl",
        "logs.journald",
        "logs.syslog-file",
        "process.procfs",
        "process.procstat",
        "process.ps",
        "socket.ss",
        "socket.sockstat",
        "socket.fstat",
        "storage.df",
        "storage.zfs",
        "package.freebsd-pkg",
        "package.openbsd-pkg-info",
        "firewall.pf",
        "privilege.sudo-helper",
        "privilege.doas-helper",
    ]
    .into_iter()
    .collect()
}

fn required_name<'a>(path: &Path, name: Option<&'a str>) -> Result<&'a str, FleetLoadError> {
    name.filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid(path, "overlay name is required for this tier"))
}

fn invalid(path: &Path, message: &str) -> FleetLoadError {
    FleetLoadError::Invalid {
        path: path.to_path_buf(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{FleetRepository, find_runtime_key, parse_inventory, parse_overlay, parse_runbook};
    use crate::{FleetOverlayTier, OperatingSystem, OperationalLayer};
    use serde_yaml::Value;
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn parses_inventory_and_runbook_documents() {
        let inventory = parse_inventory(
            Path::new("inventory.yaml"),
            r#"
id: prod-web-01
hostname: prod-web-01.example.internal
os: linux
labels:
  runlane.io/env: prod
layers:
  primary: system
  supports: [system, platform, application]
capabilities:
  requested: [os.linux, service.systemd, logs.journald, process.procfs, socket.ss, storage.df, privilege.sudo-helper]
policy:
  profile: production
"#,
        )
        .expect("inventory parses");
        assert_eq!(inventory.os, OperatingSystem::Linux);
        assert_eq!(inventory.layers.primary, OperationalLayer::System);

        let runbook = parse_runbook(
            Path::new("runbook.yaml"),
            r#"
name: service-unhealthy
version: 0.1.0
layer: system
resources:
  reads: ["system:node/{{ node }}/service/{{ service }}"]
  writes: ["system:node/{{ node }}/service/{{ service }}"]
  conflicts: ["system:node/{{ node }}/package-db"]
collect:
  - id: service_status
    capability_any: [service.systemd, service.freebsd-rc, service.openbsd-rcctl]
analyze:
  mode: structured_proposal
  allowed_actions: [service.restart, collect.more_logs]
"#,
        )
        .expect("runbook parses");
        assert_eq!(runbook.name, "service-unhealthy");
        assert!(
            runbook
                .required_capabilities
                .contains(&"service.systemd".to_owned())
        );
    }

    #[test]
    fn rejects_runtime_truth_fields() {
        let value = serde_yaml::from_str::<Value>(
            r#"
id: bad
evidence:
  source: command-output
"#,
        )
        .expect("test yaml parses");
        assert_eq!(find_runtime_key(&value), Some("evidence".to_owned()));
    }

    #[test]
    fn rejects_invalid_layer_capability_and_resource() {
        let bad_layer = parse_inventory(
            Path::new("bad.yaml"),
            r#"
id: bad
hostname: bad
os: linux
layers:
  primary: host
  supports: [system]
capabilities:
  requested: [os.linux]
policy:
  profile: production
"#,
        );
        assert!(bad_layer.is_err());

        let bad_capability = parse_runbook(
            Path::new("bad.yaml"),
            r#"
name: service-unhealthy
version: 0.1.0
layer: system
resources:
  reads: [system:node/prod/service/sshd]
collect:
  - id: bad
    capability: shell.anything
analyze:
  mode: structured_proposal
  allowed_actions: [service.restart]
"#,
        );
        assert!(bad_capability.is_err());

        let bad_resource = parse_runbook(
            Path::new("bad.yaml"),
            r#"
name: service-unhealthy
version: 0.1.0
layer: system
resources:
  reads: [node/prod/service/sshd]
collect:
  - id: status
    capability: service.systemd
analyze:
  mode: structured_proposal
  allowed_actions: [service.restart]
"#,
        );
        assert!(bad_resource.is_err());
    }

    #[test]
    fn parses_overlay_precedence_documents() {
        let overlay = parse_overlay(
            Path::new("node.yaml"),
            r#"
tier: node
name: prod-web-01
settings:
  policy.profile: node-production
"#,
        )
        .expect("overlay parses");
        assert_eq!(
            overlay.tier,
            FleetOverlayTier::Node("prod-web-01".to_owned())
        );
    }

    #[test]
    fn loads_documented_fleet_layout() {
        let root = unique_temp_dir("runlane-fleet-test");
        for directory in [
            "inventory",
            "roles",
            "runbooks",
            "policies",
            "allowlists",
            "overlays",
        ] {
            fs::create_dir_all(root.join(directory)).expect("test directory is created");
        }
        fs::write(
            root.join("inventory/prod-web-01.yaml"),
            r#"
id: prod-web-01
hostname: prod-web-01.example.internal
os: linux
labels:
  runlane.io/role: web
layers:
  primary: system
  supports: [system, platform, application]
capabilities:
  requested: [os.linux, service.systemd, logs.journald, process.procfs, socket.ss, storage.df, privilege.sudo-helper]
policy:
  profile: production
"#,
        )
        .expect("inventory fixture written");
        fs::write(
            root.join("roles/web.yaml"),
            r#"
id: web
layers:
  primary: system
runbooks:
  enabled: [service-unhealthy]
policies:
  profile: production
allowlists:
  enabled: [allow-sshd-restart]
"#,
        )
        .expect("role fixture written");
        fs::write(
            root.join("runbooks/service-unhealthy.yaml"),
            r#"
name: service-unhealthy
version: 0.1.0
layer: system
resources:
  reads: ["system:node/{{ node }}/service/{{ service }}"]
  writes: ["system:node/{{ node }}/service/{{ service }}"]
collect:
  - id: status
    capability: service.systemd
analyze:
  mode: structured_proposal
  allowed_actions: [service.restart]
"#,
        )
        .expect("runbook fixture written");
        fs::write(
            root.join("policies/production.yaml"),
            r#"
id: production
helper:
  require_signed_lease: true
  reject_replay: true
"#,
        )
        .expect("policy fixture written");
        fs::write(
            root.join("allowlists/allow-sshd-restart.yaml"),
            r#"
id: allow-sshd-restart
action: service.restart
target_resource_id: system:node/prod-web-01/service/sshd
"#,
        )
        .expect("allowlist fixture written");
        fs::write(
            root.join("overlays/global.yaml"),
            r#"
tier: global
settings:
  policy.profile: baseline
"#,
        )
        .expect("overlay fixture written");

        let fleet = FleetRepository::load(&root).expect("fleet loads");
        assert_eq!(fleet.summary().nodes, 1);
        assert_eq!(
            fleet.resolved_overlay.get("policy.profile"),
            Some("baseline")
        );
        fs::remove_dir_all(root).expect("test directory removed");
    }

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }
}
