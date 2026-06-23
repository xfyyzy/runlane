use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use runlane_core::OperatingSystem;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    pub node_id: String,
    pub server_url: String,
    pub server_trust_root_path: PathBuf,
    pub identity_path: PathBuf,
    pub certificate_path: PathBuf,
    pub private_key_path: PathBuf,
    pub spool_dir: PathBuf,
    pub platform_family: OperatingSystem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AgentConfigFile {
    node_id: String,
    server_url: String,
    server_trust_root_path: PathBuf,
    identity_path: PathBuf,
    certificate_path: PathBuf,
    private_key_path: PathBuf,
    spool_dir: PathBuf,
    platform_family: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentityMetadata {
    pub node_id: String,
    pub platform_family: OperatingSystem,
    pub certificate_fingerprint: String,
    pub server_trust_root_path: PathBuf,
    pub certificate_path: PathBuf,
    pub private_key_path: PathBuf,
    pub enrolled_at_unix_seconds: u64,
    pub expires_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AgentIdentityFile {
    node_id: String,
    platform_family: String,
    certificate_fingerprint: String,
    server_trust_root_path: PathBuf,
    certificate_path: PathBuf,
    private_key_path: PathBuf,
    enrolled_at_unix_seconds: u64,
    expires_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentState {
    pub config: AgentConfig,
    pub identity: AgentIdentityMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitConfigOptions {
    pub config_path: PathBuf,
    pub config: AgentConfig,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallIdentityOptions {
    pub config_path: PathBuf,
    pub certificate_fingerprint: String,
    pub enrolled_at_unix_seconds: u64,
    pub expires_at_unix_seconds: Option<u64>,
    pub force: bool,
}

#[derive(Debug)]
pub enum AgentConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Yaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    InvalidField {
        field: &'static str,
        reason: String,
    },
    Permission {
        path: PathBuf,
        reason: String,
    },
    Preflight {
        path: PathBuf,
        reason: String,
    },
    IdentityMismatch {
        field: &'static str,
        config_value: String,
        identity_value: String,
    },
    PlatformMismatch {
        configured: OperatingSystem,
        detected: OperatingSystem,
    },
    AlreadyExists {
        path: PathBuf,
    },
}

impl fmt::Display for AgentConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Yaml { path, source } => write!(f, "{}: invalid YAML: {source}", path.display()),
            Self::InvalidField { field, reason } => write!(f, "{field}: {reason}"),
            Self::Permission { path, reason } => {
                write!(f, "{}: unsafe permissions: {reason}", path.display())
            }
            Self::Preflight { path, reason } => write!(f, "{}: {reason}", path.display()),
            Self::IdentityMismatch {
                field,
                config_value,
                identity_value,
            } => write!(
                f,
                "identity {field} mismatch: config has {config_value:?}, identity has {identity_value:?}"
            ),
            Self::PlatformMismatch {
                configured,
                detected,
            } => write!(
                f,
                "configured platform {configured:?} does not match detected platform {detected:?}"
            ),
            Self::AlreadyExists { path } => write!(
                f,
                "{} already exists; pass --force to replace it",
                path.display()
            ),
        }
    }
}

impl Error for AgentConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Yaml { source, .. } => Some(source),
            Self::InvalidField { .. }
            | Self::Permission { .. }
            | Self::Preflight { .. }
            | Self::IdentityMismatch { .. }
            | Self::PlatformMismatch { .. }
            | Self::AlreadyExists { .. } => None,
        }
    }
}

impl AgentConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node_id: impl Into<String>,
        server_url: impl Into<String>,
        server_trust_root_path: impl Into<PathBuf>,
        identity_path: impl Into<PathBuf>,
        certificate_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
        spool_dir: impl Into<PathBuf>,
        platform_family: OperatingSystem,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            server_url: server_url.into(),
            server_trust_root_path: server_trust_root_path.into(),
            identity_path: identity_path.into(),
            certificate_path: certificate_path.into(),
            private_key_path: private_key_path.into(),
            spool_dir: spool_dir.into(),
            platform_family,
        }
    }

    fn validate_shape(&self) -> Result<(), AgentConfigError> {
        validate_node_id(&self.node_id)?;
        if !self.server_url.starts_with("https://") {
            return Err(AgentConfigError::InvalidField {
                field: "server_url",
                reason: "must use https://; plaintext agent transport is not a valid startup configuration"
                    .to_owned(),
            });
        }
        if !matches!(
            self.platform_family,
            OperatingSystem::Linux | OperatingSystem::FreeBsd | OperatingSystem::OpenBsd
        ) {
            return Err(AgentConfigError::InvalidField {
                field: "platform_family",
                reason: "must be linux, freebsd, or openbsd for v0.1 agent startup".to_owned(),
            });
        }
        for (field, path) in [
            ("server_trust_root_path", &self.server_trust_root_path),
            ("identity_path", &self.identity_path),
            ("certificate_path", &self.certificate_path),
            ("private_key_path", &self.private_key_path),
            ("spool_dir", &self.spool_dir),
        ] {
            if !path.is_absolute() {
                return Err(AgentConfigError::InvalidField {
                    field,
                    reason: "must be an absolute path so startup does not depend on cwd".to_owned(),
                });
            }
        }
        Ok(())
    }

    fn to_file(&self) -> AgentConfigFile {
        AgentConfigFile {
            node_id: self.node_id.clone(),
            server_url: self.server_url.clone(),
            server_trust_root_path: self.server_trust_root_path.clone(),
            identity_path: self.identity_path.clone(),
            certificate_path: self.certificate_path.clone(),
            private_key_path: self.private_key_path.clone(),
            spool_dir: self.spool_dir.clone(),
            platform_family: format_operating_system(self.platform_family).to_owned(),
        }
    }

    fn from_file(file: AgentConfigFile) -> Result<Self, AgentConfigError> {
        let config = Self {
            node_id: file.node_id,
            server_url: file.server_url,
            server_trust_root_path: file.server_trust_root_path,
            identity_path: file.identity_path,
            certificate_path: file.certificate_path,
            private_key_path: file.private_key_path,
            spool_dir: file.spool_dir,
            platform_family: parse_operating_system(&file.platform_family)?,
        };
        config.validate_shape()?;
        Ok(config)
    }
}

impl AgentIdentityMetadata {
    pub fn from_config(
        config: &AgentConfig,
        certificate_fingerprint: impl Into<String>,
        enrolled_at_unix_seconds: u64,
        expires_at_unix_seconds: Option<u64>,
    ) -> Result<Self, AgentConfigError> {
        let certificate_fingerprint = certificate_fingerprint.into();
        if certificate_fingerprint.trim().is_empty() {
            return Err(AgentConfigError::InvalidField {
                field: "certificate_fingerprint",
                reason: "must not be empty".to_owned(),
            });
        }
        if expires_at_unix_seconds.is_some_and(|expires| expires <= enrolled_at_unix_seconds) {
            return Err(AgentConfigError::InvalidField {
                field: "expires_at_unix_seconds",
                reason: "must be greater than enrolled_at_unix_seconds".to_owned(),
            });
        }
        Ok(Self {
            node_id: config.node_id.clone(),
            platform_family: config.platform_family,
            certificate_fingerprint,
            server_trust_root_path: config.server_trust_root_path.clone(),
            certificate_path: config.certificate_path.clone(),
            private_key_path: config.private_key_path.clone(),
            enrolled_at_unix_seconds,
            expires_at_unix_seconds,
        })
    }

    fn validate_shape(&self) -> Result<(), AgentConfigError> {
        validate_node_id(&self.node_id)?;
        if !matches!(
            self.platform_family,
            OperatingSystem::Linux | OperatingSystem::FreeBsd | OperatingSystem::OpenBsd
        ) {
            return Err(AgentConfigError::InvalidField {
                field: "identity.platform_family",
                reason: "must be linux, freebsd, or openbsd".to_owned(),
            });
        }
        if self.certificate_fingerprint.trim().is_empty() {
            return Err(AgentConfigError::InvalidField {
                field: "identity.certificate_fingerprint",
                reason: "must not be empty".to_owned(),
            });
        }
        if self
            .expires_at_unix_seconds
            .is_some_and(|expires| expires <= self.enrolled_at_unix_seconds)
        {
            return Err(AgentConfigError::InvalidField {
                field: "identity.expires_at_unix_seconds",
                reason: "must be greater than enrolled_at_unix_seconds".to_owned(),
            });
        }
        for (field, path) in [
            (
                "identity.server_trust_root_path",
                &self.server_trust_root_path,
            ),
            ("identity.certificate_path", &self.certificate_path),
            ("identity.private_key_path", &self.private_key_path),
        ] {
            if !path.is_absolute() {
                return Err(AgentConfigError::InvalidField {
                    field,
                    reason: "must be an absolute path".to_owned(),
                });
            }
        }
        Ok(())
    }

    fn to_file(&self) -> AgentIdentityFile {
        AgentIdentityFile {
            node_id: self.node_id.clone(),
            platform_family: format_operating_system(self.platform_family).to_owned(),
            certificate_fingerprint: self.certificate_fingerprint.clone(),
            server_trust_root_path: self.server_trust_root_path.clone(),
            certificate_path: self.certificate_path.clone(),
            private_key_path: self.private_key_path.clone(),
            enrolled_at_unix_seconds: self.enrolled_at_unix_seconds,
            expires_at_unix_seconds: self.expires_at_unix_seconds,
        }
    }

    fn from_file(file: AgentIdentityFile) -> Result<Self, AgentConfigError> {
        let identity = Self {
            node_id: file.node_id,
            platform_family: parse_operating_system(&file.platform_family)?,
            certificate_fingerprint: file.certificate_fingerprint,
            server_trust_root_path: file.server_trust_root_path,
            certificate_path: file.certificate_path,
            private_key_path: file.private_key_path,
            enrolled_at_unix_seconds: file.enrolled_at_unix_seconds,
            expires_at_unix_seconds: file.expires_at_unix_seconds,
        };
        identity.validate_shape()?;
        Ok(identity)
    }
}

pub fn init_config(options: &InitConfigOptions) -> Result<(), AgentConfigError> {
    options.config.validate_shape()?;
    if let Some(parent) = options.config_path.parent() {
        fs::create_dir_all(parent).map_err(|source| AgentConfigError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(&options.config.spool_dir).map_err(|source| AgentConfigError::Io {
        path: options.config.spool_dir.clone(),
        source,
    })?;
    set_private_directory_permissions(&options.config.spool_dir)?;
    write_yaml_restrictive(
        &options.config_path,
        &options.config.to_file(),
        options.force,
    )
}

pub fn install_identity(
    options: &InstallIdentityOptions,
) -> Result<AgentIdentityMetadata, AgentConfigError> {
    check_config_file_permissions(&options.config_path)?;
    let config = load_config(&options.config_path)?;
    validate_config_prerequisites(&config)?;
    let identity = AgentIdentityMetadata::from_config(
        &config,
        &options.certificate_fingerprint,
        options.enrolled_at_unix_seconds,
        options.expires_at_unix_seconds,
    )?;
    write_yaml_restrictive(&config.identity_path, &identity.to_file(), options.force)?;
    Ok(identity)
}

pub fn load_config(path: &Path) -> Result<AgentConfig, AgentConfigError> {
    let body = fs::read_to_string(path).map_err(|source| AgentConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let file = serde_yaml::from_str::<AgentConfigFile>(&body).map_err(|source| {
        AgentConfigError::Yaml {
            path: path.to_path_buf(),
            source,
        }
    })?;
    AgentConfig::from_file(file)
}

pub fn load_identity(path: &Path) -> Result<AgentIdentityMetadata, AgentConfigError> {
    let body = fs::read_to_string(path).map_err(|source| AgentConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let file = serde_yaml::from_str::<AgentIdentityFile>(&body).map_err(|source| {
        AgentConfigError::Yaml {
            path: path.to_path_buf(),
            source,
        }
    })?;
    AgentIdentityMetadata::from_file(file)
}

pub fn validate_agent_state(
    config_path: &Path,
    detected_platform: OperatingSystem,
) -> Result<AgentState, AgentConfigError> {
    check_config_file_permissions(config_path)?;
    let config = load_config(config_path)?;
    validate_config_prerequisites(&config)?;
    if config.platform_family != detected_platform {
        return Err(AgentConfigError::PlatformMismatch {
            configured: config.platform_family,
            detected: detected_platform,
        });
    }
    check_secret_file_permissions(&config.identity_path)?;
    let identity = load_identity(&config.identity_path)?;
    validate_identity_matches_config(&config, &identity)?;
    Ok(AgentState { config, identity })
}

pub fn show_config(config_path: &Path) -> Result<String, AgentConfigError> {
    check_config_file_permissions(config_path)?;
    let config = load_config(config_path)?;
    let identity_state = if config.identity_path.exists() {
        "present"
    } else {
        "missing"
    };
    Ok(format!(
        "node_id: {}\nserver_url: {}\nplatform_family: {}\nidentity_path: {}\nidentity: {}\nspool_dir: {}\n",
        config.node_id,
        config.server_url,
        format_operating_system(config.platform_family),
        config.identity_path.display(),
        identity_state,
        config.spool_dir.display()
    ))
}

fn validate_config_prerequisites(config: &AgentConfig) -> Result<(), AgentConfigError> {
    require_regular_file(&config.server_trust_root_path, "server trust root")?;
    check_public_file_permissions(&config.server_trust_root_path)?;
    require_regular_file(&config.certificate_path, "agent certificate")?;
    check_public_file_permissions(&config.certificate_path)?;
    require_regular_file(&config.private_key_path, "agent private key")?;
    check_secret_file_permissions(&config.private_key_path)?;
    require_directory(&config.spool_dir, "agent spool directory")?;
    check_directory_permissions(&config.spool_dir)?;
    Ok(())
}

fn validate_identity_matches_config(
    config: &AgentConfig,
    identity: &AgentIdentityMetadata,
) -> Result<(), AgentConfigError> {
    for (field, config_value, identity_value) in [
        ("node_id", config.node_id.clone(), identity.node_id.clone()),
        (
            "platform_family",
            format_operating_system(config.platform_family).to_owned(),
            format_operating_system(identity.platform_family).to_owned(),
        ),
        (
            "server_trust_root_path",
            config.server_trust_root_path.display().to_string(),
            identity.server_trust_root_path.display().to_string(),
        ),
        (
            "certificate_path",
            config.certificate_path.display().to_string(),
            identity.certificate_path.display().to_string(),
        ),
        (
            "private_key_path",
            config.private_key_path.display().to_string(),
            identity.private_key_path.display().to_string(),
        ),
    ] {
        if config_value != identity_value {
            return Err(AgentConfigError::IdentityMismatch {
                field,
                config_value,
                identity_value,
            });
        }
    }
    Ok(())
}

fn validate_node_id(node_id: &str) -> Result<(), AgentConfigError> {
    if node_id.trim().is_empty() {
        return Err(AgentConfigError::InvalidField {
            field: "node_id",
            reason: "must not be empty".to_owned(),
        });
    }
    if node_id
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '/' | '\\' | ':'))
    {
        return Err(AgentConfigError::InvalidField {
            field: "node_id",
            reason: "must not contain whitespace, slash, backslash, or colon".to_owned(),
        });
    }
    Ok(())
}

pub fn parse_operating_system(value: &str) -> Result<OperatingSystem, AgentConfigError> {
    match value {
        "linux" => Ok(OperatingSystem::Linux),
        "freebsd" => Ok(OperatingSystem::FreeBsd),
        "openbsd" => Ok(OperatingSystem::OpenBsd),
        "solaris" => Ok(OperatingSystem::Solaris),
        "illumos" => Ok(OperatingSystem::Illumos),
        "unknown" => Ok(OperatingSystem::Unknown),
        _ => Err(AgentConfigError::InvalidField {
            field: "platform_family",
            reason: format!("unsupported value {value:?}"),
        }),
    }
}

pub const fn format_operating_system(os: OperatingSystem) -> &'static str {
    match os {
        OperatingSystem::Linux => "linux",
        OperatingSystem::FreeBsd => "freebsd",
        OperatingSystem::OpenBsd => "openbsd",
        OperatingSystem::Solaris => "solaris",
        OperatingSystem::Illumos => "illumos",
        OperatingSystem::Unknown => "unknown",
    }
}

fn require_regular_file(path: &Path, role: &'static str) -> Result<(), AgentConfigError> {
    let metadata = fs::metadata(path).map_err(|source| AgentConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(AgentConfigError::Preflight {
            path: path.to_path_buf(),
            reason: format!("{role} must be a regular file"),
        });
    }
    Ok(())
}

fn require_directory(path: &Path, role: &'static str) -> Result<(), AgentConfigError> {
    let metadata = fs::metadata(path).map_err(|source| AgentConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(AgentConfigError::Preflight {
            path: path.to_path_buf(),
            reason: format!("{role} must be a directory"),
        });
    }
    Ok(())
}

fn check_config_file_permissions(path: &Path) -> Result<(), AgentConfigError> {
    require_regular_file(path, "agent config")?;
    reject_group_or_other_writable(path)
}

fn check_public_file_permissions(path: &Path) -> Result<(), AgentConfigError> {
    reject_group_or_other_writable(path)
}

fn check_directory_permissions(path: &Path) -> Result<(), AgentConfigError> {
    reject_group_or_other_writable(path)
}

fn check_secret_file_permissions(path: &Path) -> Result<(), AgentConfigError> {
    require_regular_file(path, "secret agent state")?;
    reject_group_or_other_accessible(path)
}

#[cfg(unix)]
fn reject_group_or_other_writable(path: &Path) -> Result<(), AgentConfigError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| AgentConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o022 != 0 {
        return Err(AgentConfigError::Permission {
            path: path.to_path_buf(),
            reason: format!("mode {mode:o} is group/other writable"),
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_group_or_other_writable(_path: &Path) -> Result<(), AgentConfigError> {
    Ok(())
}

#[cfg(unix)]
fn reject_group_or_other_accessible(path: &Path) -> Result<(), AgentConfigError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| AgentConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(AgentConfigError::Permission {
            path: path.to_path_buf(),
            reason: format!("mode {mode:o} exposes group/other access"),
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_group_or_other_accessible(_path: &Path) -> Result<(), AgentConfigError> {
    Ok(())
}

fn write_yaml_restrictive<T: Serialize>(
    path: &Path,
    value: &T,
    force: bool,
) -> Result<(), AgentConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AgentConfigError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let body = serde_yaml::to_string(value).map_err(|source| AgentConfigError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    configure_restrictive_create_mode(&mut options);
    let mut file = options.open(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::AlreadyExists {
            AgentConfigError::AlreadyExists {
                path: path.to_path_buf(),
            }
        } else {
            AgentConfigError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    file.write_all(body.as_bytes())
        .map_err(|source| AgentConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.sync_all().map_err(|source| AgentConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    set_secret_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
fn configure_restrictive_create_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(0o600);
}

#[cfg(not(unix))]
fn configure_restrictive_create_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_secret_permissions(path: &Path) -> Result<(), AgentConfigError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
        AgentConfigError::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_secret_permissions(_path: &Path) -> Result<(), AgentConfigError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), AgentConfigError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
        AgentConfigError::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), AgentConfigError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AgentConfig, AgentConfigError, AgentIdentityMetadata, InitConfigOptions,
        InstallIdentityOptions, format_operating_system, init_config, install_identity,
        validate_agent_state,
    };
    use runlane_core::OperatingSystem;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn valid_config_and_identity_validate() {
        let fixture = Fixture::new("valid");
        let config = fixture.config();
        Fixture::write_public_file(&config.server_trust_root_path);
        Fixture::write_public_file(&config.certificate_path);
        Fixture::write_secret_file(&config.private_key_path);

        init_config(&InitConfigOptions {
            config_path: fixture.config_path.clone(),
            config,
            force: false,
        })
        .expect("config init succeeds");
        let identity = install_identity(&InstallIdentityOptions {
            config_path: fixture.config_path.clone(),
            certificate_fingerprint: "sha256:demo".to_owned(),
            enrolled_at_unix_seconds: 100,
            expires_at_unix_seconds: Some(200),
            force: false,
        })
        .expect("identity install succeeds");

        let state = validate_agent_state(&fixture.config_path, OperatingSystem::Linux)
            .expect("agent state validates");
        assert_eq!(state.config.node_id, "node-valid");
        assert_eq!(state.identity, identity);
    }

    #[test]
    fn missing_config_fails_closed() {
        let fixture = Fixture::new("missing-config");
        let err = validate_agent_state(&fixture.config_path, OperatingSystem::Linux)
            .expect_err("missing config must fail");
        assert!(matches!(err, AgentConfigError::Io { .. }));
    }

    #[test]
    fn mismatched_node_identity_fails_closed() {
        let fixture = Fixture::new("mismatch");
        let config = fixture.config();
        Fixture::write_public_file(&config.server_trust_root_path);
        Fixture::write_public_file(&config.certificate_path);
        Fixture::write_secret_file(&config.private_key_path);

        init_config(&InitConfigOptions {
            config_path: fixture.config_path.clone(),
            config: config.clone(),
            force: false,
        })
        .expect("config init succeeds");
        let mut identity = AgentIdentityMetadata::from_config(&config, "sha256:demo", 100, None)
            .expect("identity shape ok");
        identity.node_id = "other-node".to_owned();
        Fixture::write_identity(&config.identity_path, &identity);

        let err = validate_agent_state(&fixture.config_path, OperatingSystem::Linux)
            .expect_err("mismatched identity must fail");
        assert!(matches!(
            err,
            AgentConfigError::IdentityMismatch {
                field: "node_id",
                ..
            }
        ));
    }

    #[test]
    fn bad_identity_permissions_fail_closed() {
        let fixture = Fixture::new("bad-perms");
        let config = fixture.config();
        Fixture::write_public_file(&config.server_trust_root_path);
        Fixture::write_public_file(&config.certificate_path);
        Fixture::write_secret_file(&config.private_key_path);

        init_config(&InitConfigOptions {
            config_path: fixture.config_path.clone(),
            config: config.clone(),
            force: false,
        })
        .expect("config init succeeds");
        let identity = AgentIdentityMetadata::from_config(&config, "sha256:demo", 100, None)
            .expect("identity shape ok");
        Fixture::write_identity(&config.identity_path, &identity);
        #[cfg(unix)]
        fs::set_permissions(&config.identity_path, fs::Permissions::from_mode(0o644))
            .expect("set bad permissions");

        let err = validate_agent_state(&fixture.config_path, OperatingSystem::Linux)
            .expect_err("readable identity file must fail closed");

        #[cfg(unix)]
        assert!(matches!(err, AgentConfigError::Permission { .. }));
        #[cfg(not(unix))]
        assert!(
            matches!(err, AgentConfigError::Io { .. })
                || matches!(err, AgentConfigError::Permission { .. })
        );
    }

    struct Fixture {
        root: PathBuf,
        config_path: PathBuf,
        suffix: String,
    }

    impl Fixture {
        fn new(suffix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "runlane-agent-config-test-{}-{suffix}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("create temp root");
            Self {
                config_path: root.join("agent.yaml"),
                root,
                suffix: suffix.to_owned(),
            }
        }

        fn config(&self) -> AgentConfig {
            AgentConfig::new(
                format!("node-{}", self.suffix),
                "https://runlane.example",
                self.root.join("trust-root.pem"),
                self.root.join("identity.yaml"),
                self.root.join("client.crt"),
                self.root.join("client.key"),
                self.root.join("spool"),
                OperatingSystem::Linux,
            )
        }

        fn write_public_file(path: &Path) {
            fs::write(path, "fixture\n").expect("write public file");
            #[cfg(unix)]
            fs::set_permissions(path, fs::Permissions::from_mode(0o644))
                .expect("set public permissions");
        }

        fn write_secret_file(path: &Path) {
            fs::write(path, "secret\n").expect("write secret file");
            #[cfg(unix)]
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .expect("set secret permissions");
        }

        fn write_identity(path: &Path, identity: &AgentIdentityMetadata) {
            let body = format!(
                "node_id: {}\nplatform_family: {}\ncertificate_fingerprint: {}\nserver_trust_root_path: {}\ncertificate_path: {}\nprivate_key_path: {}\nenrolled_at_unix_seconds: {}\nexpires_at_unix_seconds: null\n",
                identity.node_id,
                format_operating_system(identity.platform_family),
                identity.certificate_fingerprint,
                identity.server_trust_root_path.display(),
                identity.certificate_path.display(),
                identity.private_key_path.display(),
                identity.enrolled_at_unix_seconds
            );
            fs::write(path, body).expect("write identity");
            #[cfg(unix)]
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .expect("set identity permissions");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
