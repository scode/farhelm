//! The host menu's connection remedies: Retry, which tries a host's connection again now, and Adopt, which accepts the
//! identity a reinstalled or replaced host reports.
//!
//! SPEC.md, "Topology": "If a destination turns out to present a different identity than recorded ... the helm says so
//! and asks whether to adopt the new host or fix the destination — it never silently merges", adopting "resets all five
//! permissions to asking", and an entry that reaches a machine another entry holds asks the user "to remove that entry
//! or change this one's destination and then press Retry". Ported from the old app's `on_retry` and `on_adopt`
//! (`hosts.rs`) and `retry_host` and `adopt_host` (`api.rs`), at reference commit `24bc0580`. Here's the TLDR of its
//! rules:
//!
//! - **No question.** Neither asks first, as in the old menu: Retry is one more connection attempt (or, for a retired
//!   entry, a fresh connection actor), and Adopt names its identity on the menu item the user chose. Fixing the
//!   destination instead is the host settings dialog's destination field, which the menu's Settings opens.
//! - **The identity shown.** Adopt sends the identity the menu shows, which it froze when it opened ([`adopt`]'s
//!   `reported`), never one read again at the click: the helm refuses an approval of an identity the host no longer
//!   reports, so a re-probe between the menu and the click becomes a refusal the user answers by looking again, not the
//!   silent adoption of a third install.
//! - **The request** is the operation store's work under the host's key ([`Work::RetryHost`], [`Work::AdoptHost`]); the
//!   helm answers at once, without waiting on the host, and its outcome is the pushed host list.
//! - **Retry on a failed deletion.** On a host whose machine's deletion failed (a sandbox or container shown "Deleting
//!   failed"), Retry deletes it again rather than reconnecting, as the old Retry did:
//!   `POST /api/hosts/{id}/retry-delete`, chosen by the phase the host shows when Retry is pressed. That attempt can
//!   wait on the provider for longer than the UI channel allows, so the helm answers at its reply bound that the
//!   deletion carries on; that 504 is an ordinary refusal of this attempt, shown as any other, and the pushed hosts
//!   list shows how the deletion ends.
//! - **A refusal** opens the host refusal dialog ([`super::refusal`], "Retry host" or "Adopt host") through the mark of
//!   the press, and is then dropped from the store's record under the host's key; one that opened no dialog (the user
//!   moved on) stays under the row, which opens for it.
//! - **During a launch.** Both refuse to start while a session launch is in flight ([`super::launch_in_flight`]),
//!   saying "Retry <name>: not started, …" or "Adopt <name>: not started, …" on the Hosts panel's line, as the old
//!   app's page-wide lock held both. Adopt changes the registry's entry, which every host registry change waits for;
//!   Retry changes none, but on a connected host it drops the connection, which can cut short a launch's create still
//!   waiting on that host (decided by the migration's settings-and-hosts area on 2026-11-15). The line is the latest
//!   remedy refused; it ends when that remedy starts on any host, when the other is refused in its place, when Add host
//!   opens, or on a reset, and other host work starting leaves it standing.

#[cfg(test)]
mod tests;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::{HostStateView, HostView};
use gpui::{AnyWindowHandle, App, Window};
use serde_json::json;

use super::identity::display_name;
use super::listed;
use super::refusal;
use crate::focus::Mark;
use crate::host_text::HostSentence;
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::session;

/// Which of the two remedies, without what it carries: what tells the Hosts panel's "not started" lines of the two
/// apart, so a start of one clears only its own (`super::setup::remedy_not_started`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Retry,
    Adopt,
}

/// One of the two remedies, which decides its route, its body, its kind of work and its words.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Remedy {
    Retry,
    /// Adopt the identity the menu showed, raw, as the helm compares it.
    Adopt(String),
    /// Delete again a machine whose deletion failed: Retry on a host in that phase ([`retry`]).
    DeleteAgain,
}

impl Remedy {
    /// The command for host `host`: `POST /api/hosts/{id}/retry` without a body, or `POST /api/hosts/{id}/adopt` naming
    /// the identity approved.
    fn command(&self, host: i64) -> Result<Command, PathError> {
        let key = HostView::key(host);
        Ok(match self {
            Remedy::Retry => Command::new(
                Work::RetryHost,
                Method::Post,
                ApiPath::api(&["hosts", &key, "retry"])?,
                None,
                "POST /api/hosts/[host]/retry",
            ),
            Remedy::Adopt(reported) => Command::new(
                Work::AdoptHost,
                Method::Post,
                ApiPath::api(&["hosts", &key, "adopt"])?,
                Some(json!({ "reported": reported })),
                "POST /api/hosts/[host]/adopt",
            ),
            Remedy::DeleteAgain => Command::new(
                Work::RetryHost,
                Method::Post,
                ApiPath::api(&["hosts", &key, "retry-delete"])?,
                None,
                "POST /api/hosts/[host]/retry-delete",
            ),
        })
    }

    /// The refusal dialog's title action, the old dialog's words.
    fn action(&self) -> &'static str {
        match self {
            Remedy::Retry | Remedy::DeleteAgain => "Retry host",
            Remedy::Adopt(_) => "Adopt host",
        }
    }

    /// Which remedy this is, without its identity: the tag of the Hosts panel's line for it not started
    /// (`super::setup::remedy_not_started`).
    fn kind(&self) -> Action {
        match self {
            Remedy::Retry | Remedy::DeleteAgain => Action::Retry,
            Remedy::Adopt(_) => Action::Adopt,
        }
    }

    /// The verb the Hosts panel's line names a remedy not started by.
    fn verb(&self) -> &'static str {
        match self {
            Remedy::Retry | Remedy::DeleteAgain => "Retry",
            Remedy::Adopt(_) => "Adopt",
        }
    }

    /// The id of the Hosts panel's line for this remedy not started.
    fn line_id(&self) -> &'static str {
        match self {
            Remedy::Retry | Remedy::DeleteAgain => "hosts-retry-not-started",
            Remedy::Adopt(_) => "hosts-adopt-not-started",
        }
    }
}

/// The host menu's Retry for `host`, pressed with `mark` taken in its handler: a connection retry, or on a host whose
/// machine's deletion failed, that deletion again ([`deletes_again`]). While a session launch is in flight it sends
/// nothing and the Hosts panel's line says why.
pub(super) fn retry(host: i64, mark: Mark, window: &mut Window, cx: &mut App) {
    let remedy = match listed(host, cx) {
        Some(view) if deletes_again(&view.state) => Remedy::DeleteAgain,
        Some(_) | None => Remedy::Retry,
    };
    send(host, remedy, mark, window.window_handle(), cx);
}

/// Whether Retry on a host in `state` deletes its machine again rather than retrying its connection: the host shows a
/// failed deletion, the one phase in which the helm destroys a sealed machine again without a fresh question.
pub(super) fn deletes_again(state: &HostStateView) -> bool {
    matches!(state, HostStateView::Deleting { failure: Some(_) })
}

/// The host menu's Adopt for `host`, approving `reported`, the identity the menu item showed, pressed with `mark`
/// taken in its handler. While a session launch is in flight it sends nothing and the Hosts panel's line says why.
pub(super) fn adopt(host: i64, reported: String, mark: Mark, window: &mut Window, cx: &mut App) {
    send(
        host,
        Remedy::Adopt(reported),
        mark,
        window.window_handle(),
        cx,
    );
}

/// Send `remedy` for `host` under the host's key (the module docs), unless a session launch is in flight, when the
/// Hosts panel's line says it was not started. A press that meets other work on the key sends and records nothing: the
/// menu offers both only on an idle host, so only a press racing a frame gets there.
fn send(host: i64, remedy: Remedy, mark: Mark, window: AnyWindowHandle, cx: &mut App) {
    if super::launch_in_flight(cx) {
        if let Some(view) = listed(host, cx) {
            let line = HostSentence::new(remedy.line_id())
                .text(format!("{} ", remedy.verb()))
                .host(&display_name(&view))
                .text(format!(": {}", super::LAUNCH_IN_FLIGHT));
            super::setup::remedy_not_started(remedy.kind(), line, cx);
        }
        return;
    }
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let command = match remedy.command(host) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host remedy not built");
            return;
        }
    };
    let key = OpKey::host(&HostView::key(host));
    let refused_key = key.clone();
    let kind = remedy.kind();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            if let Outcome::Refused(refusal) = &settled.outcome {
                refusal::show_and_forget(
                    host,
                    remedy.action(),
                    refusal,
                    &refused_key,
                    &mark,
                    window,
                    cx,
                );
            }
        })
    });
    match started {
        // A line saying this remedy was not started is over now that one has; the other remedy's line stays.
        Ok(()) => super::setup::forget_remedy_not_started(kind, cx),
        Err(not_started) => tracing::debug!(?not_started, "host remedy not sent"),
    }
}
