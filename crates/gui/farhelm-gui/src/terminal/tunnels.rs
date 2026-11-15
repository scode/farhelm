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
//!   is read with an ordinary GET (SPEC_impl.md, "What is pushed"): the first time the pointer rests on a loopback link
//!   on a Mac ([`Tunnels::request`]), again on the next such hover after a read that failed with nothing to show
//!   ([`Tunnels::rearm`]), and after each of this terminal's own tunnel commands. Most sessions never print a loopback
//!   address, so most never read it; the hint shows once the first read answers. Nothing else creates tunnels yet.
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
//! - **Failures** are kept as one notice ([`Tunnels::notice`]) that the terminal shows under the top row until
//!   dismissed or replaced by the next tunnel action's outcome. A declined warning shows nothing.
//!
//! NOTE: Not built here yet: the pill beside the tabs with its list, closing tunnels, re-reading when consent is
//! withdrawn elsewhere, and the Settings switch's way into the warning. They come with the pill.

use farhelm_grid_model::tunnel_port;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_input_mapping::Platform;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Role, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window,
    div, px,
};
use serde::Deserialize;
use serde_json::json;

use crate::components::{Button, HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::layers::Subject;
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
    /// Why the last tunnel action failed, until dismissed or the next action starts.
    notice: Option<HostText>,
    /// Whether a read was asked for by a hover ([`Tunnels::request`]); cleared again by [`Tunnels::rearm`] after a
    /// read that failed with nothing to show.
    requested: bool,
    /// Whether the last read failed with no snapshot to show, so the next hover of a loopback link tries again.
    failed: bool,
}

impl Tunnels {
    /// The tunnels of `session`, not read yet.
    pub(crate) fn new(session: String) -> Self {
        Tunnels {
            session,
            snapshot: None,
            revision: 0,
            notice: None,
            requested: false,
            failed: false,
        }
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

    /// Why the last tunnel action failed, until dismissed or the next action starts.
    pub(crate) fn notice(&self) -> Option<&HostText> {
        self.notice.as_ref()
    }

    /// Put the notice away.
    pub(crate) fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.notice.take().is_some() {
            cx.notify();
        }
    }

    /// Read the session's snapshot. A failed read shows in the notice; the snapshot already held stays.
    fn read(&mut self, cx: &mut Context<Self>) {
        let Ok(path) = ApiPath::api(&["sessions", &self.session, "tunnels"]) else {
            return;
        };
        let revision = self.revision;
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
                            tunnels.failed = false;
                        }
                        Err(reason) => {
                            tunnels.notice = Some(reason);
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
        self.notice = None;
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
            Err(NotStarted::Busy(_)) => {
                self.notice = Some(HostText::from_host(BUSY));
                cx.notify();
            }
            Err(not_started) => tracing::debug!(?not_started, "a tunnel command was not sent"),
        }
    }

    /// A tunnel command answered.
    fn settled(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        if let Outcome::Refused(reason) = outcome {
            self.notice = Some(reason);
        }
        self.revision += 1;
        self.read(cx);
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

/// After Allow: save consent, then open `url`. A refused save shows in the notice and opens nothing.
fn allow_then_open(tunnels: WeakEntity<Tunnels>, url: String, cx: &mut App) {
    let busy = tunnels.clone();
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let Ok(path) = ApiPath::api(&["preferences"]) else {
        return;
    };
    let command = Command::new(
        Work::Save,
        Method::Put,
        path,
        Some(json!({ "tunnel_consent": true })),
        "PUT /api/preferences",
    );
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::preferences(), command, cx, move |settled, cx| {
            let outcome = settled.outcome.clone();
            let _ = tunnels.update(cx, |tunnels, cx| match outcome {
                Outcome::Refused(reason) => {
                    tunnels.notice = Some(reason);
                    cx.notify();
                }
                Outcome::Done(_) | Outcome::Accepted(_) => tunnels.send_open(url, cx),
            });
        })
    });
    match started {
        Ok(()) => {}
        // Another preferences save is in flight (an earlier Allow, or a Settings change): say so rather than nothing.
        Err(NotStarted::Busy(_)) => {
            let _ = busy.update(cx, |tunnels, cx| {
                tunnels.notice = Some(HostText::from_host(PREFERENCES_BUSY));
                cx.notify();
            });
        }
        Err(not_started) => tracing::debug!(?not_started, "tunnel consent was not saved"),
    }
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

/// The notice of the last failed tunnel action, for the terminal to draw under the top row: the reason, then Dismiss.
/// An alert, since it reports the failure of something the user just did.
pub(crate) fn notice_line(tunnels: &Entity<Tunnels>, cx: &App) -> Option<AnyElement> {
    let reason = tunnels.read(cx).notice()?.clone();
    let sentence = HostSentence::new("terminal-tunnel-notice-text").host(&reason);
    let palette = theme::palette(cx);
    let weak = tunnels.downgrade();
    Some(
        div()
            .id(NOTICE_ID)
            .debug_selector(|| NOTICE_ID.into())
            .role(Role::Alert)
            .aria_label(sentence.accessible_name())
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
            .child(div().flex_1().min_w_0().child(sentence))
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
