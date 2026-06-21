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
        "runlane-helper execute --lease-file <path> --request-file <path> --allowlist-file <path> --node-id <id> --now <unix> --dry-run"
    );
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
        ActionKind::ServiceReload
        | ActionKind::RunAllowlistedScript
        | ActionKind::RemoveAllowlistedFile => {
            Err("only service.restart dry-run is implemented in v0.1 helper".to_owned())
        }
    }
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
    use std::{
        fs,
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
    fn execute_options_require_explicit_files_node_now_and_dry_run() {
        assert!(ExecuteOptions::parse(&[]).is_err());
        let fixture = HelperFixture::write("helper-no-dry-run", "valid", 200, &[]);
        let mut args = fixture.args(100);
        args.retain(|arg| arg != "--dry-run");
        assert!(run(args).is_err());
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
        root: std::path::PathBuf,
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
                root.join("request.yaml"),
                r#"
lease_id: lease-1
action: service.restart
target_resource_id: system:node/prod-web-01/service/sshd
target_subject: sshd
arguments:
  service: sshd
"#,
            )
            .expect("request written");
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
            Self { root }
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

        fn remove(&self) {
            fs::remove_dir_all(&self.root).expect("fixture dir removed");
        }
    }

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }
}
