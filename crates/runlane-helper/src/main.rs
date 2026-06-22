use std::{env, fs, path::Path, process};

use runlane_core::{
    ActionKind, ActionTarget, CapabilityLeaseClaims, HelperActionRequest, HelperAllowlist,
    HelperAllowlistEntry, HelperArgument, HelperRejection, HelperValidationContext,
    LeaseSignatureStatus, SignedCapabilityLease, validate_helper_request,
};
use serde_yaml::Value;

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        print_skeleton();
        return Ok(());
    }

    match args.first().map(String::as_str) {
        Some("execute") => execute(&args[1..]),
        Some("dry-run-smoke") => dry_run_smoke(&args[1..]),
        Some("preflight") => preflight(&args[1..]),
        Some("--help" | "-h") => {
            print_help();
            Ok(())
        }
        _ => Err(format!(
            "unsupported runlane-helper command: {}",
            args.join(" ")
        )),
    }
}

fn print_skeleton() {
    println!(
        "runlane-helper skeleton; privileged_action_surface={:?}",
        [ActionKind::ServiceRestart, ActionKind::ServiceReload]
    );
}

fn print_help() {
    println!(
        "runlane-helper preflight --helper-binary <path> --allowlist-file <path> --expected-owner-uid <uid> --expected-mode <octal>\nrunlane-helper dry-run-smoke --lease-file <path> --request-file <path> --allowlist-file <path> --node-id <id> --now <unix>\nrunlane-helper execute --lease-file <path> --request-file <path> --allowlist-file <path> --node-id <id> --now <unix> --dry-run"
    );
}

fn preflight(args: &[String]) -> Result<(), String> {
    let options = PreflightOptions::parse(args)?;
    let report = HelperPreflightReport::check(&options)?;
    println!(
        "status: succeeded\nhelper_binary: {}\nowner_uid: {}\nmode: {:04o}\nallowlist_file: {}\ndry_run_support: present\nmessage: helper preflight passed without mutating host",
        report.helper_binary, report.owner_uid, report.mode, report.allowlist_file
    );
    Ok(())
}

fn dry_run_smoke(args: &[String]) -> Result<(), String> {
    let mut execute_args = args.to_vec();
    if !execute_args.iter().any(|arg| arg == "--dry-run") {
        execute_args.push("--dry-run".to_owned());
    }
    execute(&execute_args)
}

fn execute(args: &[String]) -> Result<(), String> {
    let options = ExecuteOptions::parse(args)?;
    if !options.dry_run {
        return Err("non-dry-run helper execution is not implemented; use --dry-run".to_owned());
    }

    let loaded = LoadedHelperInput::load(&options)?;
    let context = HelperValidationContext::new(
        options.node_id,
        options.now_unix_seconds,
        loaded.signature_status,
        loaded.seen_nonces,
        loaded.allowlist,
    );
    let accepted = validate_helper_request(&loaded.request, &loaded.lease, &context)
        .map_err(format_rejection)?;

    match accepted.action {
        ActionKind::ServiceRestart => {
            println!(
                "status: succeeded\naction: service.restart\ntarget: {}\ndry_run: true\nmessage: validated typed service.restart without mutating host",
                accepted.target.resource_id
            );
            Ok(())
        }
        ActionKind::ServiceReload | ActionKind::RemoveAllowlistedFile => {
            println!(
                "status: succeeded\naction: {:?}\ntarget: {}\ndry_run: true\nmessage: validated typed helper action without mutating host",
                accepted.action, accepted.target.resource_id
            );
            Ok(())
        }
        ActionKind::RunAllowlistedScript => {
            Err("script.run_allowlisted dry-run is not implemented in v0.1 helper".to_owned())
        }
        ActionKind::PackageUpdate | ActionKind::NodeReboot => Err(
            "package.update and node.reboot are modeled core actions, not helper-executable actions in v0.1"
                .to_owned(),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreflightOptions {
    helper_binary: String,
    allowlist_file: String,
    expected_owner_uid: u32,
    expected_mode: u32,
}

impl PreflightOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut helper_binary = None;
        let mut allowlist_file = None;
        let mut expected_owner_uid = None;
        let mut expected_mode = None;

        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--helper-binary" => {
                    helper_binary = Some(next_value(args, &mut index, "--helper-binary")?);
                }
                "--allowlist-file" => {
                    allowlist_file = Some(next_value(args, &mut index, "--allowlist-file")?);
                }
                "--expected-owner-uid" => {
                    let value = next_value(args, &mut index, "--expected-owner-uid")?;
                    expected_owner_uid = Some(value.parse::<u32>().map_err(|_| {
                        "--expected-owner-uid must be an unsigned integer".to_owned()
                    })?);
                }
                "--expected-mode" => {
                    let value = next_value(args, &mut index, "--expected-mode")?;
                    expected_mode = Some(parse_octal_mode(&value)?);
                }
                other => return Err(format!("unsupported preflight option: {other}")),
            }
        }

        Ok(Self {
            helper_binary: helper_binary.ok_or_else(|| "--helper-binary is required".to_owned())?,
            allowlist_file: allowlist_file
                .ok_or_else(|| "--allowlist-file is required".to_owned())?,
            expected_owner_uid: expected_owner_uid
                .ok_or_else(|| "--expected-owner-uid is required".to_owned())?,
            expected_mode: expected_mode.ok_or_else(|| "--expected-mode is required".to_owned())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HelperPreflightReport {
    helper_binary: String,
    owner_uid: u32,
    mode: u32,
    allowlist_file: String,
}

impl HelperPreflightReport {
    fn check(options: &PreflightOptions) -> Result<Self, String> {
        let helper_path = Path::new(&options.helper_binary);
        let metadata = fs::metadata(helper_path)
            .map_err(|error| format!("helper binary {}: {error}", helper_path.display()))?;
        if !metadata.is_file() {
            return Err(format!(
                "helper binary {} is not a regular file",
                helper_path.display()
            ));
        }
        let (owner_uid, mode) = unix_owner_and_mode(&metadata)?;
        if owner_uid != options.expected_owner_uid {
            return Err(format!(
                "helper binary {} owner uid {owner_uid} does not match expected {}",
                helper_path.display(),
                options.expected_owner_uid
            ));
        }
        if mode != options.expected_mode {
            return Err(format!(
                "helper binary {} mode {:04o} does not match expected {:04o}",
                helper_path.display(),
                mode,
                options.expected_mode
            ));
        }
        if mode & 0o111 == 0 {
            return Err(format!(
                "helper binary {} is not executable",
                helper_path.display()
            ));
        }
        if mode & 0o022 != 0 {
            return Err(format!(
                "helper binary {} must not be group- or world-writable",
                helper_path.display()
            ));
        }

        let allowlist_yaml = read_yaml(&options.allowlist_file)?;
        parse_allowlist(&allowlist_yaml)?;

        Ok(Self {
            helper_binary: options.helper_binary.clone(),
            owner_uid,
            mode,
            allowlist_file: options.allowlist_file.clone(),
        })
    }
}

fn parse_octal_mode(value: &str) -> Result<u32, String> {
    let normalized = value.strip_prefix("0o").unwrap_or(value);
    u32::from_str_radix(normalized, 8)
        .map_err(|_| format!("--expected-mode must be an octal mode, got {value:?}"))
}

#[cfg(unix)]
fn unix_owner_and_mode(metadata: &fs::Metadata) -> Result<(u32, u32), String> {
    use std::os::unix::fs::MetadataExt;

    Ok((metadata.uid(), metadata.mode() & 0o7777))
}

#[cfg(not(unix))]
fn unix_owner_and_mode(_metadata: &fs::Metadata) -> Result<(u32, u32), String> {
    Err("helper preflight owner/mode checks require a Unix platform".to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecuteOptions {
    lease_file: String,
    request_file: String,
    allowlist_file: String,
    node_id: String,
    now_unix_seconds: u64,
    dry_run: bool,
}

impl ExecuteOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut lease_file = None;
        let mut request_file = None;
        let mut allowlist_file = None;
        let mut node_id = None;
        let mut now_unix_seconds = None;
        let mut dry_run = false;

        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--lease-file" => {
                    lease_file = Some(next_value(args, &mut index, "--lease-file")?);
                }
                "--request-file" => {
                    request_file = Some(next_value(args, &mut index, "--request-file")?);
                }
                "--allowlist-file" => {
                    allowlist_file = Some(next_value(args, &mut index, "--allowlist-file")?);
                }
                "--node-id" => {
                    node_id = Some(next_value(args, &mut index, "--node-id")?);
                }
                "--now" => {
                    let value = next_value(args, &mut index, "--now")?;
                    now_unix_seconds = Some(
                        value
                            .parse::<u64>()
                            .map_err(|_| "--now must be an unsigned integer".to_owned())?,
                    );
                }
                "--dry-run" => {
                    dry_run = true;
                    index += 1;
                }
                other => return Err(format!("unsupported execute option: {other}")),
            }
        }

        Ok(Self {
            lease_file: lease_file.ok_or_else(|| "--lease-file is required".to_owned())?,
            request_file: request_file.ok_or_else(|| "--request-file is required".to_owned())?,
            allowlist_file: allowlist_file
                .ok_or_else(|| "--allowlist-file is required".to_owned())?,
            node_id: node_id.ok_or_else(|| "--node-id is required".to_owned())?,
            now_unix_seconds: now_unix_seconds.ok_or_else(|| "--now is required".to_owned())?,
            dry_run,
        })
    }
}

fn next_value(args: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    let value = args
        .get(*index + 1)
        .ok_or_else(|| format!("{option} requires a value"))?;
    *index += 2;
    Ok(value.clone())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedHelperInput {
    lease: SignedCapabilityLease,
    request: HelperActionRequest,
    allowlist: HelperAllowlist,
    signature_status: LeaseSignatureStatus,
    seen_nonces: Vec<String>,
}

impl LoadedHelperInput {
    fn load(options: &ExecuteOptions) -> Result<Self, String> {
        let lease_yaml = read_yaml(&options.lease_file)?;
        let request_yaml = read_yaml(&options.request_file)?;
        let allowlist_yaml = read_yaml(&options.allowlist_file)?;
        Ok(Self {
            lease: parse_lease(&lease_yaml)?,
            request: parse_request(&request_yaml)?,
            allowlist: parse_allowlist(&allowlist_yaml)?,
            signature_status: parse_signature_status(&lease_yaml)?,
            seen_nonces: parse_seen_nonces(&lease_yaml)?,
        })
    }
}

fn read_yaml(path: impl AsRef<Path>) -> Result<Value, String> {
    let path = path.as_ref();
    let body = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_yaml::from_str(&body)
        .map_err(|error| format!("{}: invalid yaml: {error}", path.display()))
}

fn parse_lease(value: &Value) -> Result<SignedCapabilityLease, String> {
    let claims = mapping(value, "claims")?;
    Ok(SignedCapabilityLease::new(
        CapabilityLeaseClaims::new(
            string(claims, "lease_id")?,
            string(claims, "run_id")?,
            string(claims, "approval_id")?,
            string(claims, "node_id")?,
            parse_action(&string(claims, "action")?)?,
            ActionTarget::new(
                string(claims, "target_resource_id")?,
                string(claims, "target_subject")?,
            ),
            string(claims, "allowlist_entry_id")?,
            unsigned(claims, "expires_at_unix_seconds")?,
            string(claims, "nonce")?,
        ),
        string(value, "key_id")?,
        string(value, "signature")?,
    ))
}

fn parse_request(value: &Value) -> Result<HelperActionRequest, String> {
    let arguments = value
        .get("arguments")
        .and_then(Value::as_mapping)
        .map(|mapping| {
            mapping
                .iter()
                .map(|(key, value)| {
                    Ok(HelperArgument::new(
                        key.as_str()
                            .ok_or_else(|| "argument names must be strings".to_owned())?,
                        value
                            .as_str()
                            .ok_or_else(|| "argument values must be strings".to_owned())?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(HelperActionRequest::new(
        string(value, "lease_id")?,
        parse_action(&string(value, "action")?)?,
        ActionTarget::new(
            string(value, "target_resource_id")?,
            string(value, "target_subject")?,
        ),
        arguments,
    ))
}

fn parse_allowlist(value: &Value) -> Result<HelperAllowlist, String> {
    if let Some(entries) = value.get("entries").and_then(Value::as_sequence) {
        return entries
            .iter()
            .map(parse_allowlist_entry)
            .collect::<Result<Vec<_>, _>>()
            .map(HelperAllowlist::new);
    }
    Ok(HelperAllowlist::new([parse_allowlist_entry(value)?]))
}

fn parse_allowlist_entry(value: &Value) -> Result<HelperAllowlistEntry, String> {
    Ok(HelperAllowlistEntry::new(
        string(value, "id")?,
        parse_action(&string(value, "action")?)?,
        string(value, "target_resource_id")?,
    ))
}

fn parse_signature_status(value: &Value) -> Result<LeaseSignatureStatus, String> {
    match string(value, "signature_status")?.as_str() {
        "valid" => Ok(LeaseSignatureStatus::Valid),
        "invalid" => Ok(LeaseSignatureStatus::Invalid),
        _ => Err("signature_status must be valid or invalid".to_owned()),
    }
}

fn parse_seen_nonces(value: &Value) -> Result<Vec<String>, String> {
    value
        .get("seen_nonces")
        .and_then(Value::as_sequence)
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .ok_or_else(|| "seen_nonces entries must be strings".to_owned())
                })
                .collect()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn parse_action(value: &str) -> Result<ActionKind, String> {
    match value {
        "service.restart" => Ok(ActionKind::ServiceRestart),
        "service.reload" => Ok(ActionKind::ServiceReload),
        "file.remove_from_allowlist" => Ok(ActionKind::RemoveAllowlistedFile),
        _ => Err(format!("unsupported helper action: {value}")),
    }
}

fn mapping<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing required field `{key}`"))
}

fn string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing string field `{key}`"))
}

fn unsigned(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing unsigned integer field `{key}`"))
}

fn format_rejection(rejection: HelperRejection) -> String {
    format!("helper request rejected: {rejection:?}")
}

#[cfg(test)]
mod tests {
    use super::{ExecuteOptions, LoadedHelperInput, run};
    #[cfg(unix)]
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn loads_allowlist_and_accepts_dry_run_restart() {
        let fixture = HelperFixture::write("helper-ok", "valid", 200, &[]);
        let args = fixture.args(100);
        run(args).expect("valid dry-run helper request succeeds");
        fixture.remove();
    }

    #[test]
    fn dry_run_smoke_validates_request_without_explicit_execute_flag() {
        let fixture = HelperFixture::write("helper-smoke-ok", "valid", 200, &[]);
        let mut args = vec!["dry-run-smoke".to_owned()];
        args.extend(fixture.args_without_command(100));
        args.retain(|arg| arg != "--dry-run");
        run(args).expect("dry-run smoke validates typed request");
        fixture.remove();
    }

    #[test]
    fn dry_run_accepts_typed_allowlisted_file_remove() {
        let fixture = HelperFixture::write_file_remove("helper-file-remove-ok");
        run(fixture.args(100)).expect("allowlisted file removal dry-run validates");
        fixture.remove();
    }

    #[test]
    fn rejects_invalid_expired_replayed_or_disallowed_before_action() {
        let invalid = HelperFixture::write("helper-invalid", "invalid", 200, &[]);
        assert!(run(invalid.args(100)).is_err());
        invalid.remove();

        let expired = HelperFixture::write("helper-expired", "valid", 99, &[]);
        assert!(run(expired.args(100)).is_err());
        expired.remove();

        let replayed = HelperFixture::write("helper-replayed", "valid", 200, &["nonce-1"]);
        assert!(run(replayed.args(100)).is_err());
        replayed.remove();

        let disallowed = HelperFixture::write_with_target(
            "helper-disallowed",
            "valid",
            200,
            &[],
            "system:node/prod-web-01/service/other",
        );
        assert!(run(disallowed.args(100)).is_err());
        disallowed.remove();
    }

    #[test]
    fn rejects_mismatched_lease_node_action_and_target_before_action() {
        let node = HelperFixture::write("helper-node-mismatch", "valid", 200, &[]);
        let mut node_args = node.args(100);
        let node_value = node_args
            .iter_mut()
            .skip_while(|arg| arg.as_str() != "--node-id")
            .nth(1)
            .expect("node id argument exists");
        *node_value = "other-node".to_owned();
        assert!(run(node_args).is_err());
        node.remove();

        let lease = HelperFixture::write("helper-lease-mismatch", "valid", 200, &[]);
        lease.write_request(
            "lease-other",
            "service.restart",
            "system:node/prod-web-01/service/sshd",
            "sshd",
        );
        assert!(run(lease.args(100)).is_err());
        lease.remove();

        let action = HelperFixture::write("helper-action-mismatch", "valid", 200, &[]);
        action.write_request(
            "lease-1",
            "service.reload",
            "system:node/prod-web-01/service/sshd",
            "sshd",
        );
        assert!(run(action.args(100)).is_err());
        action.remove();

        let target = HelperFixture::write("helper-target-mismatch", "valid", 200, &[]);
        target.write_request(
            "lease-1",
            "service.restart",
            "system:node/prod-web-01/service/other",
            "other",
        );
        assert!(run(target.args(100)).is_err());
        target.remove();
    }

    #[test]
    fn execute_options_require_explicit_files_node_now_and_dry_run() {
        assert!(ExecuteOptions::parse(&[]).is_err());
        let fixture = HelperFixture::write("helper-no-dry-run", "valid", 200, &[]);
        let mut args = fixture.args(100);
        args.retain(|arg| arg != "--dry-run");
        let error = run(args).expect_err("non-dry-run execution is rejected before action");
        assert!(error.contains("non-dry-run"));
        fixture.remove();
    }

    #[cfg(unix)]
    #[test]
    fn preflight_accepts_expected_helper_binary_and_allowlist() {
        let fixture = HelperFixture::write("helper-preflight-ok", "valid", 200, &[]);
        let helper_binary = fixture.write_helper_binary(0o755);
        let metadata = fs::metadata(&helper_binary).expect("helper metadata readable");
        let args = fixture.preflight_args(&helper_binary, metadata.uid(), 0o755);
        run(args).expect("preflight accepts expected binary and allowlist");
        fixture.remove();
    }

    #[cfg(unix)]
    #[test]
    fn preflight_rejects_missing_wrong_mode_writable_or_unreadable_inputs() {
        let fixture = HelperFixture::write("helper-preflight-fail", "valid", 200, &[]);
        let helper_binary = fixture.write_helper_binary(0o755);
        let metadata = fs::metadata(&helper_binary).expect("helper metadata readable");

        let missing =
            fixture.preflight_args(&fixture.root.join("missing-helper"), metadata.uid(), 0o755);
        assert!(run(missing).is_err());

        let wrong_owner =
            fixture.preflight_args(&helper_binary, metadata.uid().saturating_add(1), 0o755);
        assert!(run(wrong_owner).is_err());

        let wrong_mode = fixture.preflight_args(&helper_binary, metadata.uid(), 0o700);
        assert!(run(wrong_mode).is_err());

        let writable = fixture.write_helper_binary(0o775);
        let writable_metadata = fs::metadata(&writable).expect("helper metadata readable");
        let writable_args = fixture.preflight_args(&writable, writable_metadata.uid(), 0o775);
        assert!(run(writable_args).is_err());

        let missing_allowlist = fixture.preflight_args_with_allowlist(
            &helper_binary,
            &fixture.root.join("missing-allowlist.yaml"),
            metadata.uid(),
            0o755,
        );
        assert!(run(missing_allowlist).is_err());
        fixture.remove();
    }

    #[test]
    fn loaded_input_preserves_typed_request_shape() {
        let fixture = HelperFixture::write("helper-loaded", "valid", 200, &[]);
        let input = LoadedHelperInput::load(
            &ExecuteOptions::parse(&fixture.args(100)[1..]).expect("options parse"),
        )
        .expect("input loads");
        assert_eq!(
            input.request.action,
            runlane_core::ActionKind::ServiceRestart
        );
        assert_eq!(
            input.request.target.resource_id,
            "system:node/prod-web-01/service/sshd"
        );
        fixture.remove();
    }

    struct HelperFixture {
        root: PathBuf,
    }

    impl HelperFixture {
        fn write(prefix: &str, signature_status: &str, expires_at: u64, seen: &[&str]) -> Self {
            Self::write_with_target(
                prefix,
                signature_status,
                expires_at,
                seen,
                "system:node/prod-web-01/service/sshd",
            )
        }

        fn write_with_target(
            prefix: &str,
            signature_status: &str,
            expires_at: u64,
            seen: &[&str],
            allowlist_target: &str,
        ) -> Self {
            let root = unique_temp_dir(prefix);
            fs::create_dir_all(&root).expect("fixture dir created");
            fs::write(
                root.join("lease.yaml"),
                format!(
                    r#"
claims:
  lease_id: lease-1
  run_id: run-1
  approval_id: approval-1
  node_id: prod-web-01
  action: service.restart
  target_resource_id: system:node/prod-web-01/service/sshd
  target_subject: sshd
  allowlist_entry_id: allow-sshd-restart
  expires_at_unix_seconds: {expires_at}
  nonce: nonce-1
key_id: test-key
signature: test-signature
signature_status: {signature_status}
seen_nonces:
{}
"#,
                    seen.iter()
                        .map(|nonce| format!("  - {nonce}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            )
            .expect("lease written");
            fs::write(
                root.join("allowlist.yaml"),
                format!(
                    r#"
entries:
  - id: allow-sshd-restart
    action: service.restart
    target_resource_id: {allowlist_target}
"#
                ),
            )
            .expect("allowlist written");
            let fixture = Self { root };
            fixture.write_request(
                "lease-1",
                "service.restart",
                "system:node/prod-web-01/service/sshd",
                "sshd",
            );
            fixture
        }

        fn write_file_remove(prefix: &str) -> Self {
            let root = unique_temp_dir(prefix);
            fs::create_dir_all(&root).expect("fixture dir created");
            fs::write(
                root.join("lease.yaml"),
                r#"
claims:
  lease_id: lease-1
  run_id: run-1
  approval_id: approval-1
  node_id: prod-web-01
  action: file.remove_from_allowlist
  target_resource_id: system:node/prod-web-01/path/var-tmp-runlane-demo-cache
  target_subject: /var/tmp/runlane-demo-cache
  allowlist_entry_id: allow-runlane-demo-cache-cleanup
  expires_at_unix_seconds: 200
  nonce: nonce-1
key_id: test-key
signature: test-signature
signature_status: valid
seen_nonces: []
"#,
            )
            .expect("lease written");
            fs::write(
                root.join("request.yaml"),
                r#"
lease_id: lease-1
action: file.remove_from_allowlist
target_resource_id: system:node/prod-web-01/path/var-tmp-runlane-demo-cache
target_subject: /var/tmp/runlane-demo-cache
arguments:
  path: /var/tmp/runlane-demo-cache
"#,
            )
            .expect("request written");
            fs::write(
                root.join("allowlist.yaml"),
                r#"
entries:
  - id: allow-runlane-demo-cache-cleanup
    action: file.remove_from_allowlist
    target_resource_id: system:node/prod-web-01/path/var-tmp-runlane-demo-cache
"#,
            )
            .expect("allowlist written");
            Self { root }
        }

        fn write_request(
            &self,
            lease_id: &str,
            action: &str,
            target_resource_id: &str,
            target_subject: &str,
        ) {
            fs::write(
                self.root.join("request.yaml"),
                format!(
                    r#"
lease_id: {lease_id}
action: {action}
target_resource_id: {target_resource_id}
target_subject: {target_subject}
arguments:
  service: {target_subject}
"#
                ),
            )
            .expect("request written");
        }

        #[cfg(unix)]
        fn write_helper_binary(&self, mode: u32) -> PathBuf {
            let path = self.root.join(format!("runlane-helper-{mode:o}"));
            fs::write(&path, "# helper test fixture\n").expect("helper fixture written");
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                .expect("helper fixture permissions set");
            path
        }

        fn args(&self, now: u64) -> Vec<String> {
            vec![
                "execute".to_owned(),
                "--lease-file".to_owned(),
                self.root.join("lease.yaml").display().to_string(),
                "--request-file".to_owned(),
                self.root.join("request.yaml").display().to_string(),
                "--allowlist-file".to_owned(),
                self.root.join("allowlist.yaml").display().to_string(),
                "--node-id".to_owned(),
                "prod-web-01".to_owned(),
                "--now".to_owned(),
                now.to_string(),
                "--dry-run".to_owned(),
            ]
        }

        fn args_without_command(&self, now: u64) -> Vec<String> {
            self.args(now).into_iter().skip(1).collect()
        }

        #[cfg(unix)]
        fn preflight_args(
            &self,
            helper_binary: &std::path::Path,
            owner_uid: u32,
            mode: u32,
        ) -> Vec<String> {
            self.preflight_args_with_allowlist(
                helper_binary,
                &self.root.join("allowlist.yaml"),
                owner_uid,
                mode,
            )
        }

        #[cfg(unix)]
        fn preflight_args_with_allowlist(
            &self,
            helper_binary: &std::path::Path,
            allowlist_file: &std::path::Path,
            owner_uid: u32,
            mode: u32,
        ) -> Vec<String> {
            vec![
                "preflight".to_owned(),
                "--helper-binary".to_owned(),
                helper_binary.display().to_string(),
                "--allowlist-file".to_owned(),
                allowlist_file.display().to_string(),
                "--expected-owner-uid".to_owned(),
                owner_uid.to_string(),
                "--expected-mode".to_owned(),
                format!("{mode:o}"),
            ]
        }

        fn remove(&self) {
            fs::remove_dir_all(&self.root).expect("fixture dir removed");
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }
}
