//! Require explicit confirmation before a YOLO launch unless the host permits those
//! launches without asking.
//!
//! Every host asks before YOLO launches until the user turns off that confirmation
//! (`HostRow::yolo_without_asking`, the host settings dialog). Starting an agent that runs
//! without approval prompts on the wrong machine by accident is the failure this exists to
//! stop, so the check lives here, on every create path the helm has (a new session, a fresh
//! checkout, replace, and an agent's create and clone) and on restart-with, rather than in
//! any one client: no client can skip it. A plain restart or resume relaunches a choice the
//! user already made and is not checked.
//!
//! A refused launch is never dispatched. The browser answers the refusal
//! with a loud confirmation and retries the same request with the override
//! set; the `farhelm` command line takes an explicit flag, which the agent
//! instructions tell agents to pass only with the user's explicit approval.

use crate::AppState;
use crate::sessions::CreateMode;
use crate::store::HostId;

/// A YOLO launch refused because its host asks before YOLO launches and the request did
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
pub(crate) struct YoloNeedsConfirmation {
    pub(crate) host_name: String,
}

/// Whether a create in `mode` is a YOLO launch: a raw command by the shared
/// command-line classifier, a structured launch by its own selection.
pub(crate) fn create_is_yolo(mode: &CreateMode) -> bool {
    match mode {
        CreateMode::Raw(invocation) => invocation_is_yolo(invocation),
        CreateMode::Structured(compiled) => {
            farhelm_proto::yolo::selection_is_yolo(&compiled.selection)
        }
    }
}

/// The raw classifier lives in proto and is shared with the sidebar.
pub(crate) use farhelm_proto::yolo::invocation_is_yolo;

/// Refuse a YOLO launch on `host` unless the host allows YOLO without asking or the request
/// carries the override. Reads the host's setting from the store at the moment of the
/// decision, so a change in the settings dialog applies to the very next launch. A YOLO
/// launch to a host with no registry row is refused with the helm's unknown-host error
/// rather than allowed.
pub(crate) async fn check(
    state: &AppState,
    host: HostId,
    is_yolo: bool,
    confirm_yolo: bool,
) -> anyhow::Result<()> {
    if !is_yolo || confirm_yolo {
        return Ok(());
    }
    let row = state
        .store
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host);
    match row {
        Some(row) if row.yolo_without_asking => Ok(()),
        Some(row) => Err(anyhow::Error::new(YoloNeedsConfirmation {
            host_name: crate::aggregate::host_display_name(
                row.kind,
                row.destination.as_deref(),
                row.alias.as_deref(),
            ),
        })),
        // A host the registry no longer has is refused here, with the same
        // not-found error routing gives an unknown host
        // (`sessions::no_such_host`). Routing does not
        // cover it: create and restart-with take the host's connection
        // before this check, and host removal deletes the row before it
        // stops the connection, so a launch in that window would otherwise be
        // dispatched over the still-open connection unasked. (The wider race
        // of any create landing on a host being removed is accepted; this
        // only keeps the YOLO check from failing open.)
        None => Err(crate::sessions::no_such_host(host)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: a YOLO launch checked against a host id the registry has no row for is refused
    /// with the helm's unknown-host error ("no such host", the same text routing gives),
    /// not allowed; a launch that is not YOLO, or carries the override, is not this check's
    /// to refuse.
    ///
    /// Why: create and restart-with hold the host's connection before this check, and host
    /// removal deletes the row before stopping that connection, so a missing row does not
    /// mean routing will refuse. When this check let a missing row through, a YOLO launch
    /// in that window started an approval-free agent without the required confirmation.
    #[farhelm_testtrace::test]
    async fn a_yolo_check_against_a_host_with_no_registry_row_is_refused() {
        let harness = crate::rest_harness::idle_helm().await;
        let missing: HostId = 999_999;
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .all(|row| row.id != missing),
            "premise: no registry row has this id"
        );
        let refused = check(&harness.state, missing, true, false)
            .await
            .expect_err("a YOLO launch to a missing host must be refused");
        let refusal = refused
            .downcast_ref::<crate::SupervisorError>()
            .unwrap_or_else(|| panic!("refused with the helm's own error, got: {refused:#}"));
        assert_eq!(refusal.kind, crate::ErrorKind::NotFound);
        assert_eq!(
            refusal.message,
            format!("no such host: {missing}"),
            "the same text as any launch to an unknown host"
        );
        assert!(check(&harness.state, missing, false, false).await.is_ok());
        assert!(check(&harness.state, missing, true, true).await.is_ok());
    }
}
