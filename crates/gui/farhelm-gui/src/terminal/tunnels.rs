//! A remote session's localhost tunnels: what the Mac app's terminal offers for a loopback link, and how a Cmd-click
//! opens one.
//!
//! SPEC.md's localhost-tunnel paragraph ("In the Mac app, a loopback link in a remote session's terminal offers
//! Cmd-click to tunnel and open it in the browser") and the Security section's tunnel warning. Ported from the old
//! app's `tunnels.rs` and `terminal-links.js` (`tunnelTarget`, `shouldTunnel`, the hint in `showLinkTarget`) at
//! reference commit `24bc0580`. Here's the TLDR of the mechanics:
//!
//! - **What the helm says.** [`Tunnels`] holds the session's tunnel snapshot, `GET /api/sessions/{id}/tunnels`: whether
//!   the session's host is remote (`available`), the host's name, and the open listeners. It is not pushed state, so it
//!   is read with an ordinary GET (SPEC_impl.md, "What is pushed"): as a session on a remote host opens, so its pill
//!   lists its tunnels at once ([`Tunnels::stale`]); the first time the pointer rests on a loopback link on a Mac, for
//!   any other session ([`Tunnels::request`]), again on the next such hover after a read that failed with nothing to
//!   show ([`Tunnels::rearm`]); after each of this terminal's own tunnel commands; and whenever the shared consent
//!   changes, since withdrawing it closes every tunnel. Nothing else creates tunnels yet.
//! - **The offer.** On a Mac, a link whose host is the remote host's own loopback ([`tunnel_port`]) shows a hint in the
//!   destination display beside the pointer, naming the port and host and saying that a plain click still opens it on
//!   this Mac ([`Tunnels::hint`]). Cmd with no other modifier on a click opens it through a tunnel instead. Sessions on
//!   the local host get neither, since the helm answers `available: false` for them. The platform is a value
//!   ([`Platform`]) rather than a `cfg`, so the rule is tested on Linux too.
//! - **Consent.** The first tunnel asks the security warning ([`consent_question`]); Allow saves the shared
//!   `tunnel_consent` preference, and only then is the tunnel opened. Whether to ask is read from the pushed
//!   preference at the click, so a consent given or withdrawn in another client counts at once.
//! - **Opening.** `POST /api/sessions/{id}/tunnels/open` with the link as printed. The helm picks the local port, opens
//!   the browser at `127.0.0.1:<port>` itself with the rest of the URL kept, and reuses a tunnel already open for the
//!   same target. The GUI opens nothing. The command goes through the operation store on the session's tunnel key
//!   ([`OpKey::tunnels`]), so one tunnel action runs at a time per session, as in the old app.
//! - **The pill and its list.** With tunnels open, the session's top row shows a pill ([`pill`]): the one mapping
//!   (`:local → host:remote`, the host cut short before either port) or a count, the full mapping in its hover help and
//!   accessible name. It opens a list ([`TunnelList`]) with a close button per tunnel and Close all, which closes the
//!   tunnels shown when it was pressed, one after another, going on past failures and keeping the failed ones with their
//!   reasons. The list and closing work while the host is offline, as the helm keeps the listeners.
//! - **Failures** show in one notice ([`Tunnels::notice`], a line per failure), drawn under the top row, or inside
//!   the list while it is open, which would otherwise cover it. The last read's failure stays until a read succeeds;
//!   the last action's failures stay until the next open or close starts, and no read, failed or not, puts them away
//!   (SPEC.md: "refreshing the list does not dismiss a close failure"). Dismiss clears both. A declined warning shows
//!   nothing.
//! - **Settings' switch** asks the same warning through [`ask_consent`] and saves the same preference.
//!
//! The list takes focus as it opens, on Close all. Tab and Shift-Tab walk its controls and go on out of it, which closes
//! it without moving focus back; Escape closes it and returns focus to the pill (SPEC.md).

use farhelm_grid_model::tunnel_port;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_input_mapping::Platform;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use std::collections::VecDeque;

use farhelm_proto::ui_channel::{HostView, SessionRow};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px, svg,
};
use serde::Deserialize;
use serde_json::json;

use crate::assets::{Icon, TERMINAL_FONT};
use crate::components::{Button, HoverHelp, IconButton, MenuItem, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId, Popover, Subject};
use crate::operations::{self, Command, NotStarted, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
use crate::{session, theme};

/// What a second tunnel action says while the session's first is still in flight (the old app's words).
const BUSY: &str = "Another tunnel action is still waiting";

/// What Allow says when consent cannot be saved yet because another change to the shared preferences (an earlier
/// Allow, or a Settings change) is still being saved. Not the old app's words: its one-action rule could not meet a
/// Settings save, since Settings went through the same tunnel controller.
const PREFERENCES_BUSY: &str = "Another change to the preferences is still being saved; try again";

/// The notice's element id, and the bridge's selector for it.
pub(crate) const NOTICE_ID: &str = "terminal-tunnel-notice";

/// The id of the hint's sentence inside the destination display.
pub(crate) const HINT_ID: &str = "terminal-link-disclosure-tunnel";

/// The pill's element id, and the bridge's selector for it.
pub(crate) const PILL_ID: &str = "session-tunnel-pill";

/// The key of the anchor the list hangs from, beside the pill: an anchor's key must differ from its element's id.
const PILL_ANCHOR: &str = "session-tunnel-pill-anchor";

/// The list's element id, and the bridge's selector for it.
pub(crate) const LIST_ID: &str = "session-tunnel-list";

/// Close all's element id.
pub(crate) const CLOSE_ALL_ID: &str = "session-tunnel-close-all";

/// The prefix of each tunnel's close button id; the tunnel's id follows.
pub(crate) const CLOSE_ID_PREFIX: &str = "session-tunnel-close-";

/// The session's tunnels as `GET /api/sessions/{id}/tunnels` reports them.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct Snapshot {
    /// Whether the session's host is remote, so its loopback links are offered tunnels. The helm decides it from the
    /// host's kind, not its connectivity: an offline remote host still counts.
    pub(crate) available: bool,
    /// The host's display name, as the hint and the pill name it. Host text.
    pub(crate) host_name: String,
    /// The open listeners, in the helm's order (local port, then remote port).
    pub(crate) tunnels: Vec<Listener>,
}

/// One open tunnel: the port on this Mac's `127.0.0.1` and the remote loopback port it reaches.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct Listener {
    pub(crate) id: String,
    pub(crate) local_port: u16,
    pub(crate) remote_port: u16,
}

/// One session's tunnel state as this app knows it: the last snapshot read and the notice of the last failure.
pub(crate) struct Tunnels {
    /// The session's served id.
    session: String,
    /// The last snapshot read; `None` until the first read answers, and then no link is offered a tunnel.
    snapshot: Option<Snapshot>,
    /// Counts this terminal's own tunnel commands. A read sent before one of them answered is dropped, since it may
    /// predate what the command changed (the old app's revision rule).
    revision: u64,
    /// Why the last read failed, until a read succeeds or the notice is dismissed.
    read_error: Option<HostText>,
    /// Why the last tunnel action failed, a line per failure (Close all's failed tunnels each get one), until
    /// dismissed or the next action starts. A read, successful or not, leaves these alone (SPEC.md: "refreshing the
    /// list does not dismiss a close failure").
    failures: Vec<HostText>,
    /// The shared consent as it was when the last read was sent: a read is due again once it changes
    /// ([`Tunnels::stale`]). `None` before any read.
    consent_read: Option<Option<bool>>,
    /// Whether closes are under way (one, or Close all's run), so the list draws its closing state.
    closing: bool,
    /// The pill's focus handle, kept across frames so the list can return focus to it.
    pill_focus: FocusHandle,
    /// The list's layer while it is open.
    list: Option<LayerId>,
    /// Whether a read was asked for by a hover ([`Tunnels::request`]); cleared again by [`Tunnels::rearm`] after a
    /// read that failed with nothing to show.
    requested: bool,
    /// Whether the last read failed with no snapshot to show, so the next hover of a loopback link tries again.
    failed: bool,
}

impl Tunnels {
    /// The tunnels of `session`, not read yet.
    pub(crate) fn new(session: String, cx: &mut Context<Self>) -> Self {
        Tunnels {
            session,
            snapshot: None,
            revision: 0,
            read_error: None,
            failures: Vec::new(),
            consent_read: None,
            closing: false,
            pill_focus: cx.focus_handle().tab_stop(true),
            list: None,
            requested: false,
            failed: false,
        }
    }

    /// Whether the listing should be read now, as the store observer asks on every change of the pushed state: a
    /// session on a remote host (`remote`, by the pushed host registry) once, so its pill lists its tunnels as the view
    /// opens; and any listing read before, again, once the shared consent differs from what it was when that read was
    /// sent (withdrawing it closes every tunnel). A read still in flight counts as read, so a consent change during it
    /// sends another.
    pub(crate) fn stale(&self, remote: bool, cx: &App) -> bool {
        (remote && !self.requested)
            || self
                .consent_read
                .is_some_and(|read| read != pushed_consent(cx))
    }

    /// Read now, as [`Tunnels::stale`] asked.
    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.requested = true;
        self.read(cx);
    }

    /// Whether a failed read waits for the pointer to leave the loopback link before it may be sent again.
    pub(crate) fn retry_wanted(&self) -> bool {
        self.failed
    }

    /// Let the next hover of a loopback link read again, after a read failed with nothing to show.
    pub(crate) fn rearm(&mut self) {
        if std::mem::take(&mut self.failed) {
            self.requested = false;
        }
    }

    /// Whether a hover of a loopback link should send a read: none was asked for yet, or the last one failed and
    /// was re-armed.
    pub(crate) fn unrequested(&self) -> bool {
        !self.requested
    }

    /// Send a read, unless one was already asked for: a loopback link is under the pointer and the offer depends on it.
    pub(crate) fn request(&mut self, cx: &mut Context<Self>) {
        if !std::mem::replace(&mut self.requested, true) {
            self.read(cx);
        }
    }

    /// What the notice says, a line per failure: the last read's failure first, then the last action's; empty when
    /// there is nothing to say.
    pub(crate) fn notice(&self) -> Vec<HostText> {
        self.read_error
            .iter()
            .chain(self.failures.iter())
            .cloned()
            .collect()
    }

    /// Put the notice away, both kinds.
    pub(crate) fn dismiss(&mut self, cx: &mut Context<Self>) {
        let had = self.read_error.take().is_some() | !std::mem::take(&mut self.failures).is_empty();
        if had {
            cx.notify();
        }
    }

    /// Replace the last action's failure with one line.
    fn say(&mut self, line: HostText, cx: &mut Context<Self>) {
        self.failures = vec![line];
        cx.notify();
    }

    /// Read the session's snapshot. A failed read shows in the notice; the snapshot already held stays.
    fn read(&mut self, cx: &mut Context<Self>) {
        let Ok(path) = ApiPath::api(&["sessions", &self.session, "tunnels"]) else {
            return;
        };
        let revision = self.revision;
        self.consent_read = Some(pushed_consent(cx));
        let tunnels = cx.weak_entity();
        operations::read::<Snapshot>(
            path,
            "GET /api/sessions/[session]/tunnels",
            cx,
            move |answer, cx| {
                let _ = tunnels.update(cx, |tunnels, cx| {
                    if tunnels.revision != revision {
                        return;
                    }
                    match answer {
                        Ok(snapshot) => {
                            tunnels.snapshot = Some(snapshot);
                            tunnels.read_error = None;
                            tunnels.failed = false;
                        }
                        Err(reason) => {
                            tunnels.read_error = Some(reason);
                            tunnels.failed = tunnels.snapshot.is_none();
                        }
                    }
                    cx.notify();
                });
            },
        );
    }

    /// The tunnel hint for a link to `uri`, for the destination display beside the pointer: on a Mac, for a remote
    /// session, for a loopback link, unless the user's link choice leaves clicks to the program (`program_owns_clicks`),
    /// in which case a click does nothing of Farhelm's and there is nothing to offer. The old app's words, the host
    /// drawn as host text.
    pub(crate) fn hint(
        &self,
        uri: &str,
        platform: Platform,
        program_owns_clicks: bool,
    ) -> Option<HostSentence> {
        if platform != Platform::Mac || program_owns_clicks {
            return None;
        }
        let snapshot = self
            .snapshot
            .as_ref()
            .filter(|snapshot| snapshot.available)?;
        let port = tunnel_port(uri)?;
        Some(
            HostSentence::new(HINT_ID)
                .text(format!("This is port {port} on "))
                .host(&HostText::from_host(snapshot.host_name.as_str()))
                .text(", not on this Mac. ⌘-click to open it through a tunnel. A plain click opens it on this Mac."),
        )
    }

    /// Open `url` through a tunnel, as a Cmd-click on its link asks: after the security warning when consent has not
    /// been given, otherwise at once. `mark` is the click's, so a warning never opens after the user moved on. Only
    /// called for a link [`Tunnels::hint`] offered a tunnel for, which already means the helm said the host is remote.
    pub(crate) fn open(
        &mut self,
        url: String,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.failures.clear();
        cx.notify();
        if consented(cx) {
            self.send_open(url, cx);
            return;
        }
        let tunnels = cx.weak_entity();
        // The warning is about this terminal's tunnels: it closes, unanswered, if the terminal goes away.
        let subject = Subject::of(&cx.entity());
        prompt::ask(
            consent_question(url),
            Some(subject),
            mark,
            move |url, _, cx| allow_then_open(tunnels, url, cx),
            window,
            cx,
        );
    }

    /// Send the open command for `url` on the session's tunnel key; a refusal goes to the notice, a success re-reads.
    fn send_open(&mut self, url: String, cx: &mut Context<Self>) {
        let Ok(path) = ApiPath::api(&["sessions", &self.session, "tunnels", "open"]) else {
            return;
        };
        let command = Command::new(
            Work::TunnelOpen,
            Method::Post,
            path,
            Some(json!({ "url": url })),
            "POST /api/sessions/[session]/tunnels/open",
        );
        let key = OpKey::tunnels(&self.session);
        self.start(&key, command, cx);
    }

    /// Start `command` on `key` and settle it into this state: a refusal into the notice, a success into a fresh read.
    /// A command that cannot start because another tunnel action is in flight says so.
    fn start(&mut self, key: &OpKey, command: Command, cx: &mut Context<Self>) {
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        self.revision += 1;
        let tunnels = cx.weak_entity();
        let started = operations.update(cx, |operations, cx| {
            operations.start(key, command, cx, move |settled, cx| {
                let outcome = settled.outcome.clone();
                let _ = tunnels.update(cx, |tunnels, cx| tunnels.settled(outcome, cx));
            })
        });
        match started {
            Ok(()) => {}
            Err(NotStarted::Busy(_)) => self.say(HostText::from_host(BUSY), cx),
            Err(not_started) => tracing::debug!(?not_started, "a tunnel command was not sent"),
        }
    }

    /// A tunnel command answered.
    fn settled(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        if let Outcome::Refused(refusal) = outcome {
            self.failures = vec![refusal.words];
        }
        self.revision += 1;
        self.read(cx);
        cx.notify();
    }

    // ---- Closing ---------------------------------------------------------------------------------------------------

    /// Close `listeners`, in order, one at a time on the session's tunnel key: each that closes leaves the listing at
    /// once, each that fails stays and gets a line in the notice naming it and the helm's reason, and the listing is
    /// read again once the last has answered (the old app's Close all, which one close is a run of one of). Nothing
    /// starts while closes are already under way.
    fn close(&mut self, listeners: Vec<Listener>, cx: &mut Context<Self>) {
        if self.closing || listeners.is_empty() {
            return;
        }
        self.closing = true;
        self.failures.clear();
        cx.notify();
        self.close_next(listeners.into(), Vec::new(), cx);
    }

    /// Close the first of `queue`, then go on with the rest; `failures` holds the lines of those that failed so far.
    fn close_next(
        &mut self,
        mut queue: VecDeque<Listener>,
        mut failures: Vec<HostText>,
        cx: &mut Context<Self>,
    ) {
        let Some(listener) = queue.pop_front() else {
            self.closing = false;
            self.failures = failures;
            self.revision += 1;
            self.read(cx);
            cx.notify();
            return;
        };
        let line = |reason: &str, tunnels: &Tunnels| {
            HostText::from_host(format!("{}: {reason}", tunnels.mapping_raw(&listener)))
        };
        let Some(operations) = session::current_operations(cx) else {
            failures.push(line("not connected to the helm", self));
            return self.close_next(queue, failures, cx);
        };
        let Ok(path) = ApiPath::api(&["sessions", &self.session, "tunnels", &listener.id]) else {
            return self.close_next(queue, failures, cx);
        };
        let command = Command::new(
            Work::TunnelClose,
            Method::Delete,
            path,
            None,
            "DELETE /api/sessions/[session]/tunnels/[tunnel]",
        );
        self.revision += 1;
        let key = OpKey::tunnels(&self.session);
        let tunnels = cx.weak_entity();
        let closing = listener.clone();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, move |settled, cx| {
                let outcome = settled.outcome.clone();
                let _ = tunnels.update(cx, |tunnels, cx| {
                    match outcome {
                        Outcome::Refused(refusal) => {
                            let text = format!(
                                "{}: {}",
                                tunnels.mapping_raw(&closing),
                                refusal.words.raw()
                            );
                            failures.push(HostText::from_host(text));
                        }
                        Outcome::Done(_) | Outcome::Accepted(_) => tunnels.forget(&closing.id, cx),
                    }
                    tunnels.close_next(queue, failures, cx);
                });
            })
        });
        if let Err(not_started) = started {
            // The run stops here: the key is busy (an open sent from a hover between two closes) or the store was
            // retired, so the rest would fail the same way. Earlier failures went with the callback that never runs;
            // the read after a reset shows what is left.
            let reason = match not_started {
                NotStarted::Busy(_) => BUSY,
                NotStarted::Retired | NotStarted::Path(_) => "not connected to the helm",
            };
            self.closing = false;
            self.failures = vec![line(reason, self)];
            cx.notify();
        }
    }

    /// Drop tunnel `id` from the listing, as its close succeeded, ahead of the read that confirms it, and redraw: its
    /// row leaves the list at once.
    fn forget(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.tunnels.retain(|listener| listener.id != id);
        }
        cx.notify();
    }

    /// `listener`'s mapping as plain text, `:local → host:remote`, with the host's raw name: for a notice line, which
    /// escapes it as a whole.
    fn mapping_raw(&self, listener: &Listener) -> String {
        let host = self
            .snapshot
            .as_ref()
            .map_or("", |snapshot| snapshot.host_name.as_str());
        format!(":{} → {host}:{}", listener.local_port, listener.remote_port)
    }

    /// The open listeners, when the listing has been read; empty otherwise.
    fn listeners(&self) -> &[Listener] {
        self.snapshot
            .as_ref()
            .map_or(&[], |snapshot| snapshot.tunnels.as_slice())
    }

    /// The host's name as host text, for the pill and the list.
    fn host(&self) -> HostText {
        HostText::from_host(
            self.snapshot
                .as_ref()
                .map_or("", |snapshot| snapshot.host_name.as_str()),
        )
    }

    // ---- The list ------------------------------------------------------------------------------------------------

    /// The pill was pressed: open the list below it, or close it if it is open.
    fn toggle_list(&mut self, anchor: Anchor, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.list.filter(|list| layers::is_open(*list, window, cx)) {
            layers::close(open, window, cx);
            self.list = None;
            cx.notify();
            return;
        }
        let mark = Mark::take(cx);
        let start = cx.focus_handle().tab_stop(true);
        let tunnels = cx.weak_entity();
        let view = cx.new(|_| TunnelList {
            tunnels,
            start: start.clone(),
        });
        self.list = layers::open_popover(
            Popover {
                view: view.into(),
                container: cx.focus_handle(),
                start: Some(start),
                anchor,
                // The pill, not what held focus before: a click on the pill leaves it unfocused on macOS, and SPEC.md
                // returns focus to the pill when Escape closes the list. Naming it also gives the list its place in the
                // page's Tab order, right after the pill.
                returns_to: focus::ReturnTo::Control(self.pill_focus.clone()),
                // SPEC.md: "Tab follows its ordinary controls and leaving the list closes it without reclaiming
                // focus", where a menu would hold Tab.
                traps_tab: false,
            },
            &mark,
            window,
            cx,
        );
        cx.notify();
    }
}

/// Whether the helm's shared preference says the user accepted the tunnel warning. Only `true` counts; unset is not
/// consent. Read from the pushed preferences at the moment of the click.
fn consented(cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .is_some_and(|preferences| preferences.tunnel_consent == Some(true))
    })
}

/// The shared consent as pushed: `Some(true)` only once the warning was accepted, `None` while unset.
fn pushed_consent(cx: &App) -> Option<bool> {
    session::current_store(cx).and_then(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| preferences.tunnel_consent)
    })
}

/// Whether `session`'s host is remote by the pushed state: the session's row names its host, and the registry says it
/// is not the helm's own machine. False while either is not known.
pub(crate) fn session_is_remote(session: &str, cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        let collections = store.read(cx).collections();
        collections
            .get::<SessionRow>(session)
            .and_then(|row| collections.get::<HostView>(&HostView::key(row.host)))
            .is_some_and(|host| !host.is_local())
    })
}

/// Save the shared consent (`PUT /api/preferences`, `tunnel_consent: true`) on the preferences key, and tell `done`
/// whether it was stored: the helm's refusal, or that another preferences save is still in flight, as the reason.
/// With no UI session (the app is between connections) nothing is sent and `done` is never called: the reset that
/// ended the session already put up its own notice, and its dialogs, this question among them, are gone.
fn save_consent(cx: &mut App, done: impl FnOnce(Result<(), HostText>, &mut App) + 'static) {
    let (Some(operations), Ok(path)) = (
        session::current_operations(cx),
        ApiPath::api(&["preferences"]),
    ) else {
        return;
    };
    let command = Command::new(
        Work::Save,
        Method::Put,
        path,
        Some(json!({ "tunnel_consent": true })),
        "PUT /api/preferences",
    );
    // The completion is handed over to whichever of the two paths runs: the reply, or the refusal to start.
    let done = std::rc::Rc::new(std::cell::RefCell::new(Some(done)));
    let on_reply = done.clone();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::preferences(), command, cx, move |settled, cx| {
            let result = match &settled.outcome {
                Outcome::Refused(refusal) => Err(refusal.words.clone()),
                Outcome::Done(_) | Outcome::Accepted(_) => Ok(()),
            };
            if let Some(done) = on_reply.borrow_mut().take() {
                done(result, cx);
            }
        })
    });
    match started {
        Ok(()) => {}
        // Another preferences save is in flight (an earlier Allow, or a Settings change): say so rather than nothing.
        Err(NotStarted::Busy(_)) => {
            if let Some(done) = done.borrow_mut().take() {
                done(Err(HostText::from_host(PREFERENCES_BUSY)), cx);
            }
        }
        Err(not_started) => tracing::debug!(?not_started, "tunnel consent was not saved"),
    }
}

/// After Allow: save consent, then open `url`. A refused save shows in the notice and opens nothing.
fn allow_then_open(tunnels: WeakEntity<Tunnels>, url: String, cx: &mut App) {
    save_consent(cx, move |saved, cx| {
        let _ = tunnels.update(cx, |tunnels, cx| match saved {
            Ok(()) => tunnels.send_open(url, cx),
            Err(reason) => tunnels.say(reason, cx),
        });
    });
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Settings' tunnel switch calls it, in the settings area's own change"
    )
)]
/// Settings' way into the tunnel warning, for its switch (SPEC.md: tunnels stay off "until they accept the warning"):
/// ask the warning now, unless the user moved on since `mark`, and on Allow save the shared consent and tell `done`
/// whether it was stored. Cancel, Escape and closing the question save nothing and call nothing. Focus returns to what
/// held it when the question opened (the switch). Turning tunnels off needs no warning and is Settings' own save.
pub(crate) fn ask_consent(
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
    done: impl FnOnce(Result<(), HostText>, &mut App) + 'static,
) -> Option<LayerId> {
    prompt::ask(
        consent_question(()),
        None,
        mark,
        move |(), _, cx| save_consent(cx, done),
        window,
        cx,
    )
}

/// The security warning asked before the first tunnel opens, whose Allow acts on `frozen`: SPEC.md's Security
/// section's consequences in the old dialog's words. The old dialog's third paragraph, about links from other
/// `127.0.0.1` pages into the standalone helm, is left out: it was about the browser interface, which is gone. Focus
/// starts on Cancel, since Allow widens what a remote page can do on this Mac.
pub(crate) fn consent_question<T: Clone + 'static>(frozen: T) -> Question<T> {
    let allow = Answer {
        label: "Allow tunnels".into(),
        help: HoverHelp::tooltip(
            "Allow tunnels from remote sessions; Settings can turn them off again",
        ),
        tier: Tier::Primary,
    };
    Question::new("Allow tunnels from remote sessions?", allow, frozen)
        .summary("Remote pages share this Mac's local browser trust")
        .line(
            "A remote page opened through a tunnel runs at 127.0.0.1 on this Mac. It can read cookies set by other \
             services on 127.0.0.1; different ports do not keep those cookies separate.",
        )
        .line(
            "Your browser treats it as a local page, so its private-network checks do not apply. Any program on this \
             Mac can connect to an open tunnel.",
        )
        .line(
            "Allow this only for remote pages you trust. You can turn tunnels off in Settings, which closes every open \
             tunnel.",
        )
        .starts_on_cancel()
}

/// The notice, for the terminal to draw under the top row, while the list is closed. While the list is open the notice
/// is drawn inside it, under its heading, where the list would otherwise cover it (the old app's rule).
pub(crate) fn notice_line(
    tunnels: &Entity<Tunnels>,
    window: &Window,
    cx: &App,
) -> Option<AnyElement> {
    let open = tunnels
        .read(cx)
        .list
        .is_some_and(|list| layers::is_open(list, window, cx));
    if open {
        return None;
    }
    notice_element(tunnels, cx)
}

/// The notice of the last failures: a line per failure, then Dismiss. An alert, since it reports the failure of
/// something the user just did.
fn notice_element(tunnels: &Entity<Tunnels>, cx: &App) -> Option<AnyElement> {
    let lines = tunnels.read(cx).notice();
    if lines.is_empty() {
        return None;
    }
    let sentences: Vec<HostSentence> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            HostSentence::new(format!("terminal-tunnel-notice-text-{index}")).host(line)
        })
        .collect();
    let name: Vec<String> = sentences
        .iter()
        .map(|sentence| sentence.accessible_name().to_string())
        .collect();
    let palette = theme::palette(cx);
    let weak = tunnels.downgrade();
    Some(
        div()
            .id(NOTICE_ID)
            .debug_selector(|| NOTICE_ID.into())
            .role(Role::Alert)
            .aria_label(name.join("\n"))
            // Nothing under the notice (the terminal's text, a link in it) takes a press or a hover through it.
            .occlude()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(10.))
            .px(px(10.))
            .py(px(8.))
            .bg(theme::color(palette.danger_bg_dim))
            .border_1()
            .border_l_3()
            .border_color(theme::color(palette.danger))
            .rounded(px(3.))
            .text_color(theme::color(palette.fg_0))
            .text_size(theme::metrics::TEXT_SM)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .children(sentences),
            )
            .child(
                Button::new(
                    "terminal-tunnel-notice-dismiss",
                    "Dismiss",
                    HoverHelp::tooltip("Put this message away"),
                    Role::Button,
                )
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |tunnels, cx| tunnels.dismiss(cx));
                }),
            )
            .into_any_element(),
    )
}

// ---- The pill -------------------------------------------------------------------------------------------------------

/// The pill's widest (`.session-tunnel-pill`'s `max-width: 280px`): past it the host's name is cut short.
const PILL_MAX_WIDTH: f32 = 280.;

/// The pill's narrowest while it shows a mapping (`min-width: 154px`), so both ports stay readable beside a cut host.
const PILL_MIN_WIDTH: f32 = 154.;

/// The pill's glyphs' side (`.session-tunnel-toggle svg`: 12 pixels).
const PILL_GLYPH: f32 = 12.;

/// The pill for the session's open tunnels, for the top row's left region: `None` while there are none (SPEC.md: "No
/// pill appears when no listeners remain"). One tunnel shows its mapping, the host cut short before either port;
/// several show their count. Its hover help and accessible name carry the mapping in full. Pressing it opens or closes
/// the list. A button, so its presses stay its own inside the Mac's title band.
pub(crate) fn pill(
    tunnels: &Entity<Tunnels>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let state = tunnels.read(cx);
    let listeners = state.listeners();
    if listeners.is_empty() {
        return None;
    }
    let shows_mapping = listeners.len() == 1;
    let host = state.host();
    let open = state
        .list
        .is_some_and(|list| layers::is_open(list, window, cx));
    let focus = state.pill_focus.clone();
    let palette = theme::palette(cx);
    let ink = theme::color(if open {
        palette.on_fill
    } else {
        palette.tunnel_fg
    });
    let glyph = |icon: Icon| {
        svg()
            .path(icon.path())
            .size(px(PILL_GLYPH))
            .flex_none()
            .text_color(ink)
    };
    let (label, help, text) = match listeners {
        [one] => {
            let sentence = HostSentence::new("session-tunnel-pill-help")
                .text(format!(
                    "Open tunnels from this Mac: :{} → ",
                    one.local_port
                ))
                .host(&host)
                .text(format!(":{}", one.remote_port));
            let mapping = div()
                .flex()
                .flex_row()
                .min_w_0()
                .child(div().flex_none().child(format!(":{} → ", one.local_port)))
                .child(
                    host_text::element(&host)
                        .min_w(px(2. * 7.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap(),
                )
                .child(div().flex_none().child(format!(":{}", one.remote_port)));
            (
                sentence.accessible_name(),
                HoverHelp::sentence(sentence),
                mapping.into_any_element(),
            )
        }
        several => {
            let words = format!("{} open tunnels from this Mac", several.len());
            let count = div()
                .whitespace_nowrap()
                .child(format!("{} tunnels", several.len()));
            (
                words.clone().into(),
                HoverHelp::tooltip(words),
                count.into_any_element(),
            )
        }
    };
    let anchor = Anchor::for_element(PILL_ANCHOR, window, cx);
    let toggled = anchor.clone();
    let owner = tunnels.downgrade();
    let content = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.))
        .min_w_0()
        .font_family(TERMINAL_FONT)
        .child(glyph(Icon::Tunnel))
        .child(text)
        .child(glyph(Icon::ChevronDown));
    let button = Button::new(PILL_ID, label, help, Role::Button)
        .tier(Tier::Tunnel)
        .content(content)
        .expanded(open)
        .focus(&focus)
        .on_click(move |_, window, cx| {
            let anchor = toggled.clone();
            let _ = owner.update(cx, |tunnels, cx| tunnels.toggle_list(anchor, window, cx));
        });
    // It gives up width before the row's actions do (`flex: 0 1 auto`), down to the width that keeps a mapping's ports
    // readable, or to nothing for a count: the host's name is what gets cut short.
    Some(
        div()
            .relative()
            .flex_shrink(1.)
            .min_w_0()
            .max_w(px(PILL_MAX_WIDTH))
            .when(shows_mapping, |pill| pill.min_w(px(PILL_MIN_WIDTH)))
            .child(button)
            .child(anchor.track())
            .into_any_element(),
    )
}

// ---- The list -------------------------------------------------------------------------------------------------------

/// The open-tunnels list below the pill (`.session-tunnel-panel`, a `.host-row-menu-panel`): its heading, a row per
/// tunnel with its mapping and a close button, and Close all. It reads the session's [`Tunnels`] as it draws, so a
/// close that succeeds takes its row away at once. While closes run, the close buttons and Close all do nothing and
/// Close all reads "Closing…".
pub(crate) struct TunnelList {
    tunnels: WeakEntity<Tunnels>,
    /// Close all's handle, where focus starts.
    start: FocusHandle,
}

/// The list's width (`width: 320px`), never wider than the window less 16 pixels.
const LIST_WIDTH: f32 = 320.;

impl Render for TunnelList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let Some(tunnels) = self.tunnels.upgrade() else {
            return div().into_any_element();
        };
        let state = tunnels.read(cx);
        let host = state.host();
        let closing = state.closing;
        let listeners = state.listeners().to_vec();
        let heading = div()
            .px(px(11.))
            .pt(px(6.))
            .pb(px(8.))
            .mb(px(4.))
            .border_b_1()
            .border_color(theme::color(palette.border_dim))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child("Tunnels from this Mac"),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme::color(palette.fg_1))
                    .child("Local listeners stay open while the host is offline"),
            );
        let rows = listeners.iter().map(|listener| {
            let sentence = HostSentence::new(format!("session-tunnel-mapping-{}", listener.id))
                .text(format!(":{} → ", listener.local_port))
                .host(&host)
                .text(format!(":{}", listener.remote_port));
            let owner = self.tunnels.clone();
            let closed = listener.clone();
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .px(px(7.))
                .py(px(5.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(TERMINAL_FONT)
                        .text_size(px(11.))
                        .text_color(theme::color(palette.fg_0))
                        .child(sentence),
                )
                .child(
                    IconButton::new(
                        format!("{CLOSE_ID_PREFIX}{}", listener.id),
                        Icon::Close,
                        format!(
                            "Close tunnel from local port {} to remote port {}",
                            listener.local_port, listener.remote_port
                        ),
                        HoverHelp::tooltip("Close tunnel"),
                        Role::Button,
                    )
                    .busy(closing)
                    .on_click(move |_, _, cx| {
                        let closed = closed.clone();
                        let _ = owner.update(cx, |tunnels, cx| tunnels.close(vec![closed], cx));
                    }),
                )
        });
        let owner = self.tunnels.clone();
        let all = listeners.clone();
        let close_all = MenuItem::new(
            CLOSE_ALL_ID,
            Icon::Close,
            if closing { "Closing…" } else { "Close all" },
            HoverHelp::tooltip("Close every tunnel listed here, one after another"),
            // A button drawn as a menu command, as the old one was: the list is a group, not a menu.
            Role::Button,
        )
        .disabled(closing)
        .focus(&self.start)
        .on_click(move |_, _, cx| {
            let all = all.clone();
            let _ = owner.update(cx, |tunnels, cx| tunnels.close(all, cx));
        });
        layers::surface(cx)
            .id(LIST_ID)
            .debug_selector(|| LIST_ID.into())
            .role(Role::Group)
            .aria_label("Open tunnels from this Mac")
            .w(px(LIST_WIDTH).min(window.viewport_size().width - px(16.)))
            .max_h(window.viewport_size().height - px(88.))
            .overflow_y_scroll()
            .py(px(4.))
            .shadow(layers::menu_shadow(palette))
            .child(heading)
            .children(
                notice_element(&tunnels, cx)
                    .map(|notice| div().mx(px(7.)).mb(px(4.)).child(notice)),
            )
            .children(rows)
            .child(
                div()
                    .h(px(1.))
                    .mx(px(10.))
                    .my(px(4.))
                    .bg(theme::color(palette.border_dim)),
            )
            .child(close_all)
            .into_any_element()
    }
}
