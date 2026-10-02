//! The helm's refusal to start a YOLO session on a sensitive host without
//! an explicit override.
//!
//! Every host is sensitive until the user marks it safe for YOLO launches
//! (`HostRow::yolo_safe`, the host settings dialog). Starting an agent that
//! runs without approval prompts on the wrong machine by accident is the
//! failure this exists to stop, so the check lives here, on every create
//! path the helm has (a new session, a fresh checkout, replace, and an
//! agent's create and clone) and on restart-with, rather than in any one
//! client: no client can skip it. A plain restart or resume relaunches a
//! choice the user already made and is not checked.
//!
//! A refused launch is never dispatched. The browser answers the refusal
//! with a loud confirmation and retries the same request with the override
//! set; the `farhelm` command line takes an explicit flag, which the agent
//! instructions tell agents to pass only with the user's explicit approval.

use crate::AppState;
use crate::sessions::CreateMode;
use crate::store::HostId;

/// A YOLO launch refused because its host is sensitive and the request did
/// not carry the override.
///
/// A distinct type so the HTTP layer can recognize it (and only it) and mark
/// the response with the confirmation header the browser acts on; the
/// message is what the command line and agents show.
#[derive(Debug, thiserror::Error)]
#[error(
    "{host_name} asks before YOLO launches, and this launch skips approval prompts: nothing was \
     started. Confirm the YOLO launch explicitly (the GUI asks; the farhelm command line takes \
     --confirm-yolo), or turn on \"start YOLO sessions here without asking\" in the host's \
     settings"
)]
pub(crate) struct YoloOnSensitiveHost {
    pub(crate) host_name: String,
}

/// Whether a create in `mode` is a YOLO launch. A profile is classified by
/// the invocation of the catalog row the create resolved, the same snapshot
/// it dispatches; `do_create_session` resolves a bare profile id before
/// asking, so that arm only exists for exhaustiveness.
pub(crate) fn create_is_yolo(mode: &CreateMode) -> bool {
    match mode {
        CreateMode::Raw(invocation) => invocation_is_yolo(invocation),
        CreateMode::Structured(compiled) => {
            farhelm_proto::yolo::selection_is_yolo(&compiled.selection)
        }
        CreateMode::Profile(_) => unreachable!("do_create_session resolves profile ids first"),
        CreateMode::ResolvedProfile { profile, .. } => invocation_is_yolo(&profile.invocation),
    }
}

/// Whether a raw invocation string is a YOLO launch. An invocation the shell
/// splitter refuses is not classified here; the create itself refuses it.
pub(crate) fn invocation_is_yolo(invocation: &str) -> bool {
    shell_words::split(invocation).is_ok_and(|argv| farhelm_proto::yolo::argv_is_yolo(&argv))
}

/// Refuse a YOLO launch on `host` unless the host is marked safe or the
/// request carries the override. Reads the host's setting from the store at
/// the moment of the decision, so a change in the settings dialog applies to
/// the very next launch.
pub(crate) async fn check(
    state: &AppState,
    host: HostId,
    is_yolo: bool,
    allow_on_sensitive_host: bool,
) -> anyhow::Result<()> {
    if !is_yolo || allow_on_sensitive_host {
        return Ok(());
    }
    let row = state
        .store
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host);
    match row {
        Some(row) if row.yolo_safe => Ok(()),
        Some(row) => Err(anyhow::Error::new(YoloOnSensitiveHost {
            host_name: crate::aggregate::host_display_name(
                row.kind,
                row.destination.as_deref(),
                row.alias.as_deref(),
            ),
        })),
        // A host the registry no longer has is not this check's to report;
        // routing to it refuses on its own.
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: every built-in profile named `…-yolo` is classified YOLO by the
    /// sensitive-host guard, and every other built-in is not.
    ///
    /// Why: built-in profiles are plain command lines, so the guard sees them
    /// only through the shared classifier's tables. When the classifier
    /// stopped reading the generic name `agent`, the built-in `cursor-yolo`
    /// profile (then `agent --force`) would have started on a sensitive host
    /// without asking had its launch not moved to `cursor-agent` in the same
    /// change. An edit to either the built-in table or the classifier tables
    /// can break that coupling silently; this catches it.
    #[test]
    fn builtin_yolo_profiles_are_guarded_and_the_rest_are_not() {
        let builtins = crate::store::builtin_profiles();
        assert!(
            builtins
                .iter()
                .any(|profile| profile.name.ends_with("-yolo")),
            "premise: some built-in YOLO profile exists"
        );
        for profile in builtins {
            assert_eq!(
                invocation_is_yolo(&profile.invocation),
                profile.name.ends_with("-yolo"),
                "built-in profile {} ({})",
                profile.name,
                profile.invocation
            );
        }
    }
}
