//! `records backup --dest <dir>`: a fast-forward-only mirror of the records repo in a local bare repo.
//!
//! The records repo lives in Cloudflare Artifacts, which documents nothing about refusing force pushes or ref
//! deletion, so a bad push could rewrite history there. The backup is the guard: it fetches every branch with the
//! refspec `refs/heads/*:refs/heads/*` (no leading `+`), so git refuses any update that is not a fast-forward and the
//! backup keeps the history it already had. A refusal, like every other failure, exits 1 and sends one notification
//! when the host has an ntfy topic file (`~/.farhelm-ntfy-topic`), through the checkout's
//! `scripts/plans-landing/notify.sh` (the same send every plans notification uses). The message is one line in product
//! terms with no paths, because ntfy.sh is a third party.
//!
//! It assumes nothing about the host beyond the config's `remote` and `[artifacts]` table and the API token file: it
//! mints its own short-lived `read` repo token for each run (`POST {api_base}/accounts/{account_id}/artifacts/
//! namespaces/{namespace}/tokens` with `{"repo", "scope": "read", "ttl": 600}`), so the host needs no repo token, and
//! the minted token goes to git as a Bearer header the same way slot clones send theirs. No slot is needed: backup can
//! run from any Farhelm checkout.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::config::{self, Artifacts, Config};
use super::git::Git;
use super::{Context, Failure};

/// Lifetime of the minted read token: long enough for a slow fetch, short enough that a leaked one is worth little.
const READ_TOKEN_TTL_SECONDS: u64 = 600;

/// Why a backup failed, in the words of the notification. Each is one line with no paths or host details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    Setup,
    Mint,
    Fetch,
    Rewritten,
}

impl Cause {
    fn notification(self) -> &'static str {
        match self {
            Self::Setup => {
                "agent-records backup failed: the backup could not start (config, token file or backup repo)"
            }
            Self::Mint => {
                "agent-records backup failed: could not mint a read token from Cloudflare"
            }
            Self::Fetch => "agent-records backup failed: could not fetch from the records repo",
            Self::Rewritten => {
                "agent-records backup refused a rewritten history in the records repo; the backup kept the old history"
            }
        }
    }
}

/// Runs the backup and, when it fails, sends the failure notification before returning the failure.
pub fn run_and_notify(context: &Context, config_path: &Path, dest: &Path) -> Result<(), Failure> {
    let result = run(context, config_path, dest);
    if let Err((cause, _)) = &result {
        notify(context, cause.notification());
    }
    result.map_err(|(_, failure)| failure)
}

fn run(context: &Context, config_path: &Path, dest: &Path) -> Result<(), (Cause, Failure)> {
    let config: Config =
        config::load(config_path, context.home.as_deref()).map_err(|f| (Cause::Setup, f))?;
    run_with(&config, config_path, dest).map_err(|(cause, failure)| {
        // The remote URL and the mint URL both carry the Cloudflare account id; neither belongs in a message that may
        // be copied into a log or report.
        let mut failure = failure.redact_remote(&config.remote);
        if let Some(artifacts) = &config.artifacts {
            failure.message = failure
                .message
                .replace(&artifacts.account_id, "<account id>");
        }
        (cause, failure)
    })
}

/// The backup proper, once the config is loaded. `dest` is absolute (the command line insists).
fn run_with(config: &Config, config_path: &Path, dest: &Path) -> Result<(), (Cause, Failure)> {
    let setup = |f: Failure| (Cause::Setup, f);
    let artifacts = config.artifacts.as_ref().ok_or_else(|| {
        setup(Failure::failed(format!(
            "{} has no [artifacts] table, which backup needs",
            config_path.display()
        )))
    })?;
    let api_token =
        config::read_secret(&artifacts.api_token_file, "api_token_file").map_err(setup)?;
    let token = mint_read_token(artifacts, &api_token).map_err(|f| (Cause::Mint, f))?;
    let git = Git::new(Some(token));
    ensure_bare_repo(&git, dest).map_err(setup)?;

    // --porcelain makes each ref's outcome a machine-readable line, so a refused non-fast-forward is told apart from
    // a transport failure without matching prose.
    let output = git
        .at(dest)
        .args([
            "fetch",
            "--porcelain",
            "--no-tags",
            "--no-write-fetch-head",
            &config.remote,
            "refs/heads/*:refs/heads/*",
        ])
        .output()
        .map_err(|f| (Cause::Fetch, f))?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let rewritten = stdout.lines().any(|line| line.starts_with('!'));
    let cause = if rewritten {
        Cause::Rewritten
    } else {
        Cause::Fetch
    };
    let what = if rewritten {
        "the records repo's history was rewritten; the backup refused the non-fast-forward update and is unchanged"
    } else {
        "fetching the records repo failed"
    };
    Err((
        cause,
        Failure::failed(format!("{what}: {} {}", stdout.trim(), stderr.trim())),
    ))
}

/// Creates the bare backup repo on first use; refuses an existing directory that is not a bare repo.
fn ensure_bare_repo(git: &Git, dest: &Path) -> Result<(), Failure> {
    if !dest.exists() {
        let parent = dest.parent().unwrap_or(Path::new("/"));
        fs::create_dir_all(parent)
            .map_err(|e| Failure::failed(format!("cannot create {}: {e}", parent.display())))?;
        git.at(parent)
            .args(["init", "--bare", "-q", "--initial-branch=main"])
            .args([dest])
            .run()?;
        return Ok(());
    }
    let output = git
        .at(dest)
        .args(["rev-parse", "--is-bare-repository"])
        .output()?;
    if output.status.success() && output.text() == "true" {
        Ok(())
    } else {
        Err(Failure::failed(format!(
            "{} exists but is not a bare git repo",
            dest.display()
        )))
    }
}

/// Mints a `read` repo token through the Cloudflare REST API. The API token goes only into the request's
/// Authorization header; neither token appears in an error message.
fn mint_read_token(artifacts: &Artifacts, api_token: &str) -> Result<String, Failure> {
    let url = format!(
        "{}/accounts/{}/artifacts/namespaces/{}/tokens",
        artifacts.api_base, artifacts.account_id, artifacts.namespace
    );
    let body = serde_json::json!({ "repo": artifacts.repo, "scope": "read", "ttl": READ_TOKEN_TTL_SECONDS }).to_string();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        // The API's error envelope explains a 4xx better than the status alone, so read the body on every status.
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into();
    let mut response = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {api_token}"))
        .header("Content-Type", "application/json")
        .send(&body)
        .map_err(|e| Failure::failed(format!("minting a read token at {url} failed: {e}")))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().map_err(|e| {
        Failure::failed(format!("reading the token response from {url} failed: {e}"))
    })?;
    parse_mint_response(status, &text)
        .map_err(|message| Failure::failed(format!("minting a read token at {url}: {message}")))
}

/// Extracts the token from Cloudflare's v4 envelope: `{"success": true, "errors": [], "result": {"plaintext": ...}}`.
///
/// The token create route returns the token string as `result.plaintext` (with `id`, `scope` and `expires_at` beside
/// it), per Cloudflare's Artifacts REST API docs. On failure the envelope's `errors[].message` texts are reported, never
/// the raw body, which might carry a token.
fn parse_mint_response(status: u16, body: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| format!("HTTP {status} with a response that is not JSON"))?;
    let success = value.get("success").and_then(serde_json::Value::as_bool) == Some(true);
    let token = value
        .pointer("/result/plaintext")
        .and_then(serde_json::Value::as_str)
        .filter(|t| !t.is_empty());
    match token {
        Some(token) if success && (200..300).contains(&status) => {
            if token.chars().any(char::is_control) {
                Err("the minted token contains control characters".to_string())
            } else {
                Ok(token.to_string())
            }
        }
        _ => {
            let errors: Vec<String> = value
                .get("errors")
                .and_then(serde_json::Value::as_array)
                .map(|errors| {
                    errors
                        .iter()
                        .map(|e| {
                            let code = e.get("code").map(|c| c.to_string()).unwrap_or_default();
                            let message = e
                                .get("message")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("");
                            format!("{code} {message}").trim().to_string()
                        })
                        .collect()
                })
                .unwrap_or_default();
            if errors.is_empty() {
                Err(format!(
                    "HTTP {status}, success={success}, and no token in result.plaintext"
                ))
            } else {
                Err(format!("HTTP {status}: {}", errors.join("; ")))
            }
        }
    }
}

/// Sends `message` through the checkout's `notify.sh` when the host has an ntfy topic file. A failed send is reported
/// on stderr and does not change the exit status, which already says the backup failed.
///
/// The script is found in the checkout containing the working directory, the one `cargo xtask` runs from.
fn notify(context: &Context, message: &str) {
    let Some(home) = &context.home else { return };
    if !home.join(".farhelm-ntfy-topic").is_file() {
        return;
    }
    let script =
        config::find_checkout(&context.cwd).map(|c| c.join("scripts/plans-landing/notify.sh"));
    let Some(script) = script.filter(|s| s.is_file()) else {
        eprintln!(
            "records: could not send the failure notification: no scripts/plans-landing/notify.sh in this checkout"
        );
        return;
    };
    match Command::new(&script).arg(message).status() {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("records: the failure notification was not sent ({status})"),
        Err(e) => eprintln!("records: could not run {}: {e}", script.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented success envelope yields `result.plaintext` whole, including its `?expires=` suffix, which the
    /// Bearer header needs.
    #[test]
    fn success_envelope_yields_plaintext() {
        let body = r#"{"result":{"id":"0123456789abcdef","plaintext":"art_v1_abc?expires=1760000000","scope":"read",
            "expires_at":"2026-01-01T00:00:00Z"},"success":true,"errors":[],"messages":[]}"#;
        assert_eq!(
            parse_mint_response(200, body).unwrap(),
            "art_v1_abc?expires=1760000000"
        );
    }

    /// Failures report the envelope's error codes and messages and never echo the body, and a "success" without a
    /// token, a non-2xx status or a non-JSON body are failures too.
    #[test]
    fn failures_are_explained_without_the_body() {
        let error = r#"{"result":null,"success":false,"errors":[{"code":10000,"message":"Authentication error"}],"messages":[]}"#;
        assert_eq!(
            parse_mint_response(403, error).unwrap_err(),
            "HTTP 403: 10000 Authentication error"
        );
        let no_token = r#"{"result":{"plaintext":""},"success":true,"errors":[]}"#;
        assert!(parse_mint_response(200, no_token).is_err());
        let wrong_status = r#"{"result":{"plaintext":"art_v1_secret"},"success":true,"errors":[]}"#;
        assert!(
            !parse_mint_response(500, wrong_status)
                .unwrap_err()
                .contains("secret")
        );
        assert!(
            !parse_mint_response(200, "art_v1_secret not json")
                .unwrap_err()
                .contains("secret")
        );
        let token_field = r#"{"result":{"token":"art_v1_secret"},"success":true,"errors":[]}"#;
        assert!(
            parse_mint_response(200, token_field).is_err(),
            "only result.plaintext is the documented field"
        );
    }

    /// Every notification is one line without paths, as ntfy.sh is a third party and the topic shows the message as is.
    #[test]
    fn notifications_are_single_lines() {
        for cause in [Cause::Setup, Cause::Mint, Cause::Fetch, Cause::Rewritten] {
            let text = cause.notification();
            assert!(!text.contains('\n') && !text.contains('/'), "{text}");
        }
    }
}
