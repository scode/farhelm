//! Recipe admission returns ordinary host-progress headers, then one JSON result.
//! The request owns its long transport budget through the body read. No retry or
//! progress failure can turn an uncertain environment launch into a second launch.

use super::*;

/// A resource may exist after any transport or decoding failure. Always give
/// the user the existing Hosts recovery surface before another creation attempt.
fn uncertain(reason: impl std::fmt::Display) -> SandboxLaunchError {
    SandboxLaunchError::Unknown(format!(
        "{reason}. Check Hosts for the environment before launching again."
    ))
}

/// A environment launch can outlive its transport. Keep uncertainty distinct from
/// a definite refusal so the composer never claims a running task did not launch.
#[derive(Debug)]
pub(crate) enum SandboxLaunchError {
    Refused(String),
    Unknown(String),
}

impl SandboxLaunchError {
    /// Unknown results require checking Hosts before another creation attempt.
    pub(crate) fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl std::fmt::Display for SandboxLaunchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(text) | Self::Unknown(text) => formatter.write_str(text),
        }
    }
}

/// Ordinary launch encoding with placement deliberately omitted. An older
/// ordinary-create route must never receive this body's recipe intent.
pub(crate) fn sandbox_launch_body(
    cwd: &str,
    agent: CreateAgent<'_>,
    title: &str,
    key: &str,
) -> serde_json::Value {
    let mut body = create_body(cwd, agent, title, key, None, None);
    body.as_object_mut()
        .expect("create body is an object")
        .remove("host");
    body
}

/// Submit captured recipe/place intent without an installed-host selector.
/// Admission exposes the existing provisioning run; the final body decides success.
pub(crate) async fn create_environment_session(
    base: &str,
    target: &farhelm_proto::LaunchTarget,
    confirmed: bool,
    name_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    request: serde_json::Value,
    admitted: impl FnOnce(HostId, String),
) -> Result<Session, SandboxLaunchError> {
    let recipe = target.recipe();
    let place = target.place();
    let url = format!("{base}/api/environment-sessions");
    let mut deadline = LaunchDeadline::default();
    let response = send_inner_marked(
        client().post(&url).json(&serde_json::json!({
            "recipe": recipe, "place": place, "confirmed": confirmed, "name_kind": name_kind, "request": request,
        })),
        std::time::Duration::from_secs(30 * 60),
        true,
        Some(&mut deadline),
    )
    .await
    .map_err(|error| uncertain(send_error_text(error)))?;
    if !response.status().is_success() {
        return Err(SandboxLaunchError::Refused(
            refusal_text("POST", &url, response).await,
        ));
    }
    let host = response
        .headers()
        .get("x-farhelm-environment-host")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<HostId>().ok())
        .ok_or_else(|| uncertain("The environment admission did not identify its host"))?;
    let run = response
        .headers()
        .get("x-farhelm-environment-run")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            value.len() == 36
                && value.bytes().enumerate().all(|(index, byte)| {
                    if [8, 13, 18, 23].contains(&index) {
                        byte == b'-'
                    } else {
                        byte.is_ascii_hexdigit()
                    }
                })
        })
        .ok_or_else(|| uncertain("The environment admission did not identify its setup run"))?
        .to_string();
    admitted(host, run);
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Reply {
        Session {
            session: Box<Session>,
        },
        Error {
            error: String,
            #[serde(default)]
            uncertain: bool,
        },
    }
    match response
        .json::<Reply>()
        .await
        .map_err(|error| uncertain(send_error_text(deadline.error(error))))?
    {
        Reply::Session { session } => Ok(*session),
        Reply::Error { error, uncertain } => Err(if uncertain {
            SandboxLaunchError::Unknown(error)
        } else {
            SandboxLaunchError::Refused(error)
        }),
    }
}

/// This consent deliberately authorizes every process and file in the resource,
/// independently of the selected session's cached liveness or tab count.
pub(crate) async fn delete_sandbox_session(
    base: &str,
    id: &str,
) -> Result<farhelm_proto::SessionDeleteOutcome, String> {
    let url = format!(
        "{base}/api/sessions/{}?delete_sandbox=true",
        encode_path_segment(id)
    );
    let response = send_within(client().delete(&url), std::time::Duration::from_secs(180)).await?;
    if !response.status().is_success() {
        return Err(refusal_text("DELETE", &url, response).await);
    }
    // Whole-resource cleanup completed before these success headers. Its
    // body carries no checkout-preservation facts and cannot earn an archive
    // cue or the ordinary Delete decoder's lost-checkout-notice warning.
    let _ = response.bytes().await;
    Ok(farhelm_proto::SessionDeleteOutcome::default())
}

/// The helm owns both phases after admission, so closing a page after Stop
/// cannot abandon the requested Suspend. A failed Stop never freezes peers.
pub(crate) async fn stop_and_suspend(
    base: &str,
    id: &str,
    no_tabs: bool,
    ended: bool,
) -> Result<(), String> {
    let url = format!(
        "{base}/api/sessions/{}/stop-and-suspend",
        encode_path_segment(id)
    );
    let response = send_within(
        client().post(&url).json(&serde_json::json!({
            "refuse_if_tabs_open": no_tabs, "refuse_if_agent_running": ended,
        })),
        std::time::Duration::from_secs(180),
    )
    .await?;
    if !response.status().is_success() {
        return Err(refusal_text("POST", &url, response).await);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An installed host or connection token would bind this recipe launch to
    /// another machine. Both must be absent, rather than null or a local ID.
    #[farhelm_testtrace::test]
    fn sandbox_body_omits_installed_placement_without_losing_command_intent() {
        let command = farhelm_proto::CommandLaunch {
            command: "printf ready".into(),
            yolo: false,
            agent: None,
            resume: None,
        };
        let body = sandbox_launch_body("~", CreateAgent::Command(&command), "work", "intent");
        assert!(body.get("host").is_none());
        assert!(body.get("expected_incarnation").is_none());
        assert_eq!(body["cwd"], "~");
        assert_eq!(body["command"], serde_json::to_value(command).unwrap());
        assert_eq!(body["title"], "work");
    }

    /// A transport problem can leave billed work behind. Ordinary create's
    /// safe-retry promise must never be borrowed by a recipe launch.
    #[farhelm_testtrace::test]
    fn uncertain_recipe_errors_point_to_hosts_without_promising_safe_retry() {
        let failure = uncertain("Connection lost");
        assert!(failure.is_unknown());
        let message = failure.to_string();
        assert!(message.contains("Check Hosts"));
        assert!(message.contains("before launching again"));
        assert!(!message.contains("will not start a second"));
    }
}
