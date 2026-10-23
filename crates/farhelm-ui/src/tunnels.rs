//! Native tunnel consent and actions share one warning and one acknowledged write.
//!
//! The helm remains authoritative. This controller prevents a stale terminal or
//! expired card from continuing after a warning, and never opens a URL itself:
//! the native opener belongs to the helm's tunnel operation.

use dioxus::prelude::*;
use farhelm_proto::approvals::PendingApproval;
use futures_channel::oneshot;

use crate::ApiBase;
use crate::api::{self, SessionTunnels};
use crate::feed::{fallback_polls_now, fallback_sleep};
use crate::list::SharedPreferences;
use crate::modal_dialog::{
    DialogFocus, DialogHandle, ModalDialog, RecoveryDelay, RecoveryFocus, ScrollRegion,
};
use crate::ops::{ConfirmSlot, ConfirmToken, use_confirm_slot};
use crate::peer::display_peer;
use crate::reader::{SurfaceReader, Trigger, request_read};

/// The controller survives the warning and releases it for each source's close.
const DIALOG: DialogHandle = DialogHandle::new(".tunnel-consent-dialog[role=\"dialog\"]");
const SETTINGS: &str = ".app-settings-dialog[role=\"dialog\"]";
/// Settings stays mounted while consent temporarily owns the modal boundary.
const SETTINGS_DIALOG: DialogHandle = DialogHandle::new(SETTINGS);

/// The action's lifetime follows its source, rather than the selected pane alone.
///
/// Also the key of the security warning's [`ConfirmSlot`], where it records
/// which source's action an opening's Allow completes. It does not decide
/// which showing an answer reaches: the slot's token check compares whole
/// tokens, and the opening number alone already tells showings apart.
/// `PartialEq` is there because that check compares the key too.
///
/// NOTE: The derived equality compares every field, so for a card it
/// includes the expiry, host name and session details. It is not the
/// liveness check: [`TunnelController::current`] and
/// [`TunnelController::observe_cards`] match a card by request id, host,
/// session id and action only.
#[derive(Clone, PartialEq, Debug)]
enum Source {
    Terminal { session: String, generation: u64 },
    Card(Box<PendingApproval>),
    Settings,
}

/// The shared security warning: at most one opening, keyed by the [`Source`]
/// whose action waits on it, holding the sender that tells that action's
/// preparation whether the user allowed tunnels.
///
/// A warning owns only its result; the caller retains the intent until
/// acceptance. Dropping the sender (a reconciliation `clear`, or the slot
/// going away) reads as "not allowed" to the waiting preparation, exactly
/// like sending `Ok(false)`, because it awaits with `unwrap_or(Ok(false))`.
type WarningSlot = ConfirmSlot<Source, oneshot::Sender<Result<bool, String>>>;

/// One showing of the security warning, as the render that drew its Allow,
/// Cancel and Escape captured it.
type WarningToken = ConfirmToken<Source>;

/// Page-owned controller, provided above both cards and keyed session views.
#[derive(Clone, Copy)]
pub(crate) struct TunnelController {
    busy: Signal<bool>,
    warning: WarningSlot,
    saving: Signal<bool>,
    view: Signal<Option<(String, u64)>>,
    next_view: Signal<u64>,
    revision: Signal<u64>,
    notice: Signal<Option<String>>,
    preferences: SharedPreferences,
}

/// Install context in the preferences gate so it survives selection changes.
pub(crate) fn use_controller() {
    let preferences = use_context::<SharedPreferences>();
    let warning = use_confirm_slot();
    use_context_provider(|| TunnelController::new(warning, preferences));
}

impl TunnelController {
    /// A controller with no action, warning or save in flight. Creates its
    /// signals in the calling scope, so call it once per owner: from
    /// [`use_controller`]'s provider, or from a test's hook.
    fn new(warning: WarningSlot, preferences: SharedPreferences) -> Self {
        Self {
            busy: Signal::new(false),
            warning,
            saving: Signal::new(false),
            view: Signal::new(None),
            next_view: Signal::new(0),
            revision: Signal::new(0),
            notice: Signal::new(None),
            preferences,
        }
    }

    /// A newly selected view supersedes earlier terminal intents immediately.
    /// Clear the old view before cancelling its modal, so focus cannot return to it.
    fn enter_view(mut self, session: &str) -> u64 {
        self.view.set(None);
        let terminal_warning = self
            .warning
            .peek_key()
            .is_some_and(|source| matches!(source, Source::Terminal { .. }));
        // An accepted warning whose save is still running survives this;
        // `retire_warning` explains why that still suppresses the open.
        if terminal_warning {
            self.retire_warning();
        }
        let generation = self.next_view.peek().wrapping_add(1);
        self.next_view.set(generation);
        self.view.set(Some((session.to_string(), generation)));
        generation
    }

    /// Retire only terminal intents when their keyed view leaves or changes owner.
    fn leave_view(mut self, generation: u64) {
        if self
            .view
            .peek()
            .as_ref()
            .is_some_and(|(_, current)| *current == generation)
        {
            self.view.set(None);
            if self.warning.peek_key().is_some_and(|source|
                matches!(source, Source::Terminal { generation: own, .. } if own == generation))
            {
                self.retire_warning();
            }
        }
    }

    /// A card must still name the same request and host after every async boundary.
    async fn current(self, base: &str, source: &Source) -> Result<bool, String> {
        match source {
            Source::Terminal {
                session,
                generation,
            } => Ok(self
                .view
                .peek()
                .as_ref()
                .is_some_and(|(id, active)| id == session && active == generation)),
            Source::Card(card) => Ok(api::fetch_approvals(base).await?.iter().any(|live| {
                live.id == card.id
                    && live.host_id == card.host_id
                    && live.session.id == card.session.id
                    && live.action == card.action
            })),
            Source::Settings => Ok(true),
        }
    }

    /// Admit one action, then consult current server consent before requesting the warning.
    /// The caller releases the claim after its final create/answer, preventing duplicate opens.
    async fn prepare(mut self, base: &str, source: Source) -> Result<bool, String> {
        if *self.busy.peek() {
            return Err("another tunnel action is still waiting".into());
        }
        self.busy.set(true);
        let result = self.prepare_claimed(base, source).await;
        if !matches!(result, Ok(true)) {
            self.busy.set(false);
        }
        result
    }

    /// Preparation holds no optimistic consent; only the server can authorize creation.
    async fn prepare_claimed(mut self, base: &str, source: Source) -> Result<bool, String> {
        if !self.current(base, &source).await? {
            return Ok(false);
        }
        let session = match &source {
            Source::Terminal { session, .. } => Some(session.as_str()),
            Source::Card(card) => Some(card.session.id.as_str()),
            Source::Settings => None,
        };
        let consent = if let Some(session) = session {
            let state = api::fetch_tunnels(base, session).await?;
            if !state.available {
                return Err("tunneling is unavailable for this session".into());
            }
            self.preferences.0.write().tunnel_consent = Some(state.consent);
            state.consent
        } else {
            false
        };
        if !self.current(base, &source).await? {
            return Ok(false);
        }
        if !consent {
            let (tx, rx) = oneshot::channel();
            // The returned token is dropped: the dialog's render reads the
            // opening back with `current` and hands it to its answers.
            self.warning.open(source.clone(), tx);
            if !rx.await.unwrap_or(Ok(false))? {
                return Ok(false);
            }
        }
        self.current(base, &source).await
    }

    /// Consent for a card does not answer it. Its existing handler keeps expiry and errors.
    pub(crate) async fn prepare_card(
        self,
        base: &str,
        card: PendingApproval,
    ) -> Result<bool, String> {
        self.prepare(base, Source::Card(Box::new(card))).await
    }

    /// The existing card reader supplies both feed and fallback expiry observations.
    /// Do not add a second fleet-wide polling surface for an open warning.
    pub(crate) fn observe_cards(self, cards: &[PendingApproval]) {
        let vanished = self.warning.peek_key().is_some_and(|source| {
            if let Source::Card(card) = &source {
                !cards.iter().any(|live| {
                    live.id == card.id
                        && live.host_id == card.host_id
                        && live.session.id == card.session.id
                        && live.action == card.action
                })
            } else {
                false
            }
        });
        if vanished {
            self.retire_warning();
        }
    }

    /// Release a completed create/answer and invalidate native tunnel lists explicitly.
    pub(crate) fn complete(mut self) {
        self.busy.set(false);
        self.changed();
    }

    /// Both Settings choices belong to the app, including a warning's pending result.
    /// Claim before yielding so queued clicks cannot start competing actions. Closing
    /// Settings retires only its result receiver; this task still releases busy and
    /// reports failures globally when no originating control remains to display them.
    fn settings_choice(
        mut self,
        base: String,
        enable: bool,
    ) -> oneshot::Receiver<Result<(), String>> {
        self.busy.set(true);
        let (tx, rx) = oneshot::channel();
        dioxus::core::spawn_forever(async move {
            let result = if enable {
                self.prepare_claimed(&base, Source::Settings)
                    .await
                    .map(|_| ())
            } else {
                api::write_tunnel_consent(&base, false).await.map(|()| {
                    self.preferences.0.write().tunnel_consent = Some(false);
                })
            };
            self.complete();
            if let Err(Err(error)) = tx.send(result) {
                self.notice.set(Some(error));
            }
        });
        rx
    }

    /// Invalidate a list read begun before a successful local mutation.
    fn changed(mut self) {
        let revision = self.revision.peek().wrapping_add(1);
        self.revision.set(revision);
    }

    /// Accept an Allow drawn for `shown`, returning the source whose action
    /// the save completes: the one that showing was opened for, never
    /// whichever warning is open when the click runs.
    ///
    /// Refused (`None`, nothing changed) while an accepted save is running,
    /// so a second Allow from the same burst starts no second save, and when
    /// `shown` is no longer the open showing (cancelled, retired or replaced
    /// since its render), so a stale Allow neither saves consent nor lets
    /// another source's action proceed, and leaves the warning now on screen
    /// as it is.
    ///
    /// Accepting does not consume the opening. The dialog stays up, its
    /// buttons disabled, through the consent write, and [`Self::finish_save`]
    /// retires the opening with that write's outcome. This is the
    /// stay-up-through-the-request exception `ConfirmSlot`'s docs describe;
    /// `saving` is what keeps a second answer from starting a second save,
    /// and what makes Cancel, Escape and reconciliation leave the accepted
    /// opening alone meanwhile.
    fn accept_warning(mut self, shown: &WarningToken) -> Option<Source> {
        if *self.saving.peek() || !self.warning.is_current(shown) {
            return None;
        }
        self.saving.set(true);
        Some(shown.key().clone())
    }

    /// The accepted warning retains busy ownership through its save, even if
    /// its source disappears. Preparation rechecks that source before opening.
    /// `shown` is the token [`Self::accept_warning`] accepted; the outcome goes
    /// to that opening's preparation.
    fn finish_save(mut self, shown: &WarningToken, result: Result<bool, String>) {
        self.saving.set(false);
        self.finish_warning(shown, result);
    }

    /// Answer the showing `shown` names with `result`, then close the dialog:
    /// Cancel and Escape (`Ok(false)`) and a finished save. Does nothing when
    /// `shown` is no longer the open showing, so a dismissal drawn for an
    /// earlier showing cannot close, or move focus out of, the warning now on
    /// screen.
    ///
    /// Release the owned warning before retiring its result and returning focus.
    /// Preparation retains ownership of the original action, so result delivery
    /// still precedes the framed return.
    fn finish_warning(mut self, shown: &WarningToken, result: Result<bool, String>) {
        // A queued Cancel can arrive after Allow starts saving but before DOM
        // disabling renders. Only finish_save may retire that accepted warning.
        if *self.saving.peek() || !self.warning.is_current(shown) {
            return;
        }
        DIALOG.release();
        let Some(reply) = self.warning.take(shown) else {
            return;
        };
        let _ = reply.send(result);
        self.close_dialog(shown.key());
    }

    /// Reconciliation: close whatever warning is open because its source went
    /// away (its terminal view left or was superseded, its card expired).
    /// Callers decide from [`ConfirmSlot::peek_key`] that the open warning is
    /// the one to retire; this is never an answer, which is why it takes no
    /// token.
    ///
    /// Leaves an accepted warning alone while its save runs, so callers need
    /// not check `saving` themselves. Once Allow starts saving, the warning
    /// stays up until that save completes, so its outcome (a failed write
    /// included, which the preparation reports) reaches the opening that was
    /// answered. Skipping retirement does not let a gone source's action run:
    /// preparation rechecks the source after the warning.
    ///
    /// The preparation waiting on the warning sees "not allowed": the slot's
    /// `clear` drops its result sender (see [`WarningSlot`]).
    fn retire_warning(mut self) {
        if *self.saving.peek() {
            return;
        }
        let Some(source) = self.warning.peek_key() else {
            return;
        };
        DIALOG.release();
        self.warning.clear();
        self.close_dialog(&source);
    }

    /// Return focus after the warning for `source` closed.
    ///
    /// Settings restoration and source focus remain framed so the closing
    /// render can remove the child first.
    fn close_dialog(self, source: &Source) {
        // Settings resumes isolation before taking focus. Terminal and card
        // returns below remain separate frame submissions, in their original
        // order, because their source lifetime chooses a different destination.
        DIALOG.close(&format!("
            const settings = document.querySelector({settings});
            if (settings) {{ {install} settings.querySelector('.tunnel-settings-switch')?.focus({{preventScroll:true}}); }}
        ",
            settings = serde_json::to_string(SETTINGS).unwrap(),
            install = SETTINGS_DIALOG.isolation_script(Some(".app-settings-close"))));
        if matches!(source, Source::Terminal { session, generation }
            if self.view.peek().as_ref().is_some_and(|(active, epoch)| active == session && epoch == generation))
        {
            document::eval(
                "requestAnimationFrame(() => document.querySelector('.terminal-pane.selected .xterm-helper-textarea')?.focus({preventScroll:true}))",
            );
        } else if let Source::Card(card) = source {
            let id = serde_json::to_string(&card.id).unwrap();
            document::eval(&format!(
                "requestAnimationFrame(() => [...document.querySelectorAll('.approval-card')].find(card => card.dataset.approvalId === {id})?.querySelector('.approval-tunnel')?.focus({{preventScroll:true}}))"
            ));
        }
    }
}

/// Install the native gesture bridge and render the single shared security warning.
#[component]
pub(crate) fn TunnelControls() -> Element {
    let mut controller = use_context::<TunnelController>();
    let base = use_context::<ApiBase>().0;
    let bridge_base = base.clone();
    use_future(move || {
        let base = bridge_base.clone();
        async move {
            #[derive(serde::Deserialize)]
            struct Intent {
                session_id: String,
                url: String,
                generation: u64,
            }
            let mut channel = document::eval(
                "window.farhelmTunnelOpen = intent => dioxus.send(intent); await new Promise(() => {});",
            );
            while let Ok(intent) = channel.recv::<Intent>().await {
                let Some((session, generation)) = controller.view.peek().clone() else {
                    continue;
                };
                if session != intent.session_id || generation != intent.generation {
                    continue;
                }
                let base = base.clone();
                spawn(async move {
                    let source = Source::Terminal {
                        session: session.clone(),
                        generation,
                    };
                    match controller.prepare(&base, source.clone()).await {
                        Ok(true) => {
                            let result = if controller.current(&base, &source).await == Ok(true) {
                                api::open_tunnel(&base, &session, &intent.url).await
                            } else {
                                Ok(())
                            };
                            if let Err(error) = result {
                                controller.notice.set(Some(error));
                            }
                            controller.complete();
                        }
                        Ok(false) => {}
                        Err(error) => controller.notice.set(Some(error)),
                    }
                });
            }
            // A dead receive channel must not leave an apparently usable bridge.
            document::eval("delete window.farhelmTunnelOpen;");
        }
    });
    use_drop(move || {
        document::eval("delete window.farhelmTunnelOpen;");
    });
    // The dialog draws the opening this render read and moves that token into
    // Escape, Cancel and Allow, so each answer names the showing it was drawn
    // for. The text is the same for every source; the token's source decides
    // which action an Allow lets proceed.
    let shown = controller.warning.current();
    rsx! {
        if let Some(error) = controller.notice.read().as_ref() {
            // Persists until dismissed above terminal link destination
            // previews (app.css's z-index ladder), so it is a link cover: a
            // link beside it whose preview it overlaps refuses the click.
            div { class: "tunnel-error-notice", role: "alert", "data-link-cover": "true",
                span { "{display_peer(error)}" }
                button { r#type: "button", class: "btn btn-neutral", onclick: move |_| controller.notice.set(None), "dismiss" }
            }
        }
        if let Some(shown) = shown {
            div { class: "host-settings-backdrop tunnel-consent-backdrop", role: "presentation",
                ModalDialog {
                    handle: DIALOG,
                    before_install_release: SETTINGS_DIALOG,
                    focus: DialogFocus::remembered(".tunnel-cancel", Some(".tunnel-cancel"), false, Some(".tunnel-allow")),
                    class: "tunnel-consent-dialog host-dialog-framed", role: "dialog", aria_modal: "true",
                    aria_labelledby: "tunnel-consent-heading", tabindex: "-1",
                    on_escape: {
                        let shown = shown.clone();
                        move |event: KeyboardEvent| {
                            if !event.is_composing() && !*controller.saving.peek() {
                                event.stop_propagation(); controller.finish_warning(&shown, Ok(false));
                            }
                        }
                    },
                    ScrollRegion { class: "host-settings-body",
                        handle: DIALOG, selector: ".host-settings-body", name: "tunnel security warning",
                        recovery: RecoveryFocus::new(".tunnel-cancel", None, true, RecoveryDelay::OneFrame),
                        h2 { id: "tunnel-consent-heading", "Allow tunnels from remote sessions?" }
                        p { strong { "A remote page opened through a tunnel runs at 127.0.0.1 on this Mac. It can read cookies set by other services on 127.0.0.1; different ports do not keep those cookies separate." } }
                        p { "Your browser treats it as a local page, so its private-network checks do not apply. Any program on this Mac can connect to an open tunnel." }
                        p { "Ordinary links from other 127.0.0.1 pages into the standalone Farhelm helm are refused." }
                        p { "Allow this only for remote pages you trust. You can turn tunnels off in Settings, which closes every open tunnel." }
                        div { class: "approval-card-actions",
                            button { r#type: "button", class: "btn btn-neutral tunnel-cancel", disabled: (controller.saving)(),
                                onclick: {
                                    let shown = shown.clone();
                                    move |_| controller.finish_warning(&shown, Ok(false))
                                }, "Cancel" }
                            button { r#type: "button", class: "btn btn-primary tunnel-allow", disabled: (controller.saving)(),
                                onclick: {
                                    let shown = shown.clone();
                                    move |_| {
                                        let Some(source) = controller.accept_warning(&shown) else { return; };
                                        let base = base.clone();
                                        let shown = shown.clone();
                                        spawn(async move {
                                            let result = match controller.current(&base, &source).await {
                                                Ok(true) => api::write_tunnel_consent(&base, true).await.map(|()| {
                                                    controller.preferences.0.write().tunnel_consent = Some(true); true
                                                }),
                                                Ok(false) => Ok(false),
                                                Err(error) => Err(error),
                                            };
                                            controller.finish_save(&shown, result);
                                        });
                                    }
                                }, "Allow tunnels" }
                        }
                    }
                }
            }
        }
    }
}

/// Native-only shared switch. No optimistic checked state can stand in for an acknowledgment.
#[component]
pub(crate) fn TunnelSettings() -> Element {
    let mut controller = use_context::<TunnelController>();
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut error = use_signal(|| None::<String>);
    let mut reading = use_signal(|| true);
    use_future(move || {
        let base = read_base.clone();
        async move {
            match api::fetch_preferences(&base).await {
                Ok(preferences) => {
                    controller.preferences.0.write().tunnel_consent = preferences.tunnel_consent
                }
                Err(reason) => error.set(Some(reason)),
            }
            reading.set(false);
        }
    });
    let allowed = controller.preferences.0.read().tunnel_consent == Some(true);
    rsx! {
        label { class: "app-settings-choice",
            span { "allow tunnels from remote sessions" }
            input { r#type: "checkbox", class: "host-settings-switch tunnel-settings-switch",
                checked: allowed, disabled: (controller.busy)() || reading(), aria_describedby: "tunnel-settings-help",
                onchange: move |event| {
                    let enable = event.checked();
                    // The native checkbox has already toggled. Keep its visible value
                    // at the last acknowledged choice while the request is pending.
                    document::eval(&format!("document.querySelector('.tunnel-settings-switch').checked = {allowed};"));
                    if *controller.busy.peek() { return; }
                    error.set(None);
                    let completion = controller.settings_choice(base.clone(), enable);
                    spawn(async move {
                        match completion.await {
                            Ok(Ok(())) => {},
                            Ok(Err(reason)) => error.set(Some(reason)),
                            Err(_) => error.set(Some("the tunnel setting could not be saved; try again".into())),
                        }
                    });
                }
            }
        }
        p { id: "tunnel-settings-help", class: "host-settings-help",
            "Open a remote session's localhost links through this Mac. Turning this off closes every open tunnel. The next tunnel asks again. This choice applies to this helm."
        }
        if let Some(error) = error.read().as_ref() {
            p { class: "app-settings-error", role: "alert", "{display_peer(error)}" }
        }
    }
}

/// Read and close this session's local resources through the existing feed/backstop discipline.
/// Kept outside the stale-session inert subtree so offline listeners remain closeable.
#[component]
pub(crate) fn TunnelStrip(session: String) -> Element {
    let mut controller = use_context::<TunnelController>();
    let base = use_context::<ApiBase>().0;
    let mut state = use_signal(SessionTunnels::default);
    let mut error = use_signal(|| None::<String>);
    let mut closing = use_signal(|| None::<String>);
    let surface = use_signal(SurfaceReader::default);
    let generation = use_hook(|| controller.enter_view(&session));
    let read_base = base.clone();
    let read_session = session.clone();
    let read = move || {
        let base = read_base.clone();
        let session = read_session.clone();
        let revision = *controller.revision.peek();
        async move {
            match api::fetch_tunnels(&base, &session).await {
                Ok(fresh) => {
                    if *controller.revision.peek() != revision {
                        return false;
                    }
                    controller.preferences.0.write().tunnel_consent = Some(fresh.consent);
                    state.set(fresh);
                    error.set(None);
                    true
                }
                Err(reason) => {
                    error.set(Some(reason));
                    false
                }
            }
        }
    };
    let request = move |trigger| request_read(surface, trigger, read.clone());
    let mount = request.clone();
    use_hook(move || mount(Trigger::Explicit));
    let notice = request.clone();
    crate::feed::use_feed_reader(move || notice(Trigger::Notice));
    let changed = request.clone();
    use_effect(move || {
        let _ = (controller.revision)();
        changed(Trigger::Explicit);
    });
    let poll = request.clone();
    use_future(move || {
        let poll = poll.clone();
        async move {
            loop {
                fallback_sleep().await;
                if fallback_polls_now() {
                    poll(Trigger::Scheduled);
                }
            }
        }
    });
    let publish_session = session.clone();
    use_effect(move || {
        // The terminal hover escapes host names itself. Publish raw wire text
        // so this bridge does not escape it twice or change its lookup snapshot.
        let mut value = serde_json::to_value(&*state.read()).unwrap();
        value["generation"] = serde_json::json!(generation);
        let id = serde_json::to_string(&publish_session).unwrap();
        document::eval(&format!(
            "window.farhelmTunnelSessions ||= Object.create(null); window.farhelmTunnelSessions[{id}] = {value};"
        ));
    });
    let cleanup_session = session.clone();
    use_drop(move || {
        controller.leave_view(generation);
        let id = serde_json::to_string(&cleanup_session).unwrap();
        document::eval(&format!(
            "if (window.farhelmTunnelSessions?.[{id}]?.generation === {generation}) delete window.farhelmTunnelSessions[{id}];"
        ));
    });
    rsx! {
        if !state.read().tunnels.is_empty() {
            div { class: "session-tunnels", aria_label: "open tunnels from this Mac",
                span { "Tunnels from this Mac:" }
                for tunnel in state.read().tunnels.iter() {
                    span { key: "{tunnel.id}", class: "session-tunnel-chip",
                        span { ":{tunnel.local_port} → " }
                        span { class: "peer-value", dir: "ltr", "{state.read().host_name.escaped()}" }
                        span { ":{tunnel.remote_port}" }
                        button { r#type: "button", class: "session-tunnel-close",
                            aria_label: "close tunnel from local port {tunnel.local_port} to remote port {tunnel.remote_port}",
                            disabled: closing.read().is_some(), onclick: {
                                let base = base.clone(); let session = session.clone(); let id = tunnel.id.clone(); let request = request.clone();
                                move |_| {
                                    if closing.peek().is_some() { return; }
                                    closing.set(Some(id.clone()));
                                    let base = base.clone(); let session = session.clone(); let id = id.clone(); let request = request.clone();
                                    spawn(async move {
                                        match api::close_tunnel(&base, &session, &id).await {
                                            Ok(()) => { state.write().tunnels.retain(|tunnel| tunnel.id != id); error.set(None); controller.changed(); }
                                            Err(reason) => error.set(Some(reason)),
                                        }
                                        closing.set(None); request(Trigger::Explicit);
                                    });
                                }
                            }, "×"
                        }
                    }
                }
            }
        }
        if let Some(error) = error.read().as_ref() {
            p { class: "session-tunnel-error", role: "alert", "{display_peer(error)}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    /// Run `body` against a fresh controller mounted in a probe component.
    ///
    /// The warning slot has to come from a hook, so a probe builds the
    /// controller on its first render and hands it out through a
    /// thread-local. `body` then runs after the render, inside the probe's
    /// scope, because the dialog helpers the controller calls on close
    /// (`DIALOG.release`, `document::eval`) need a current scope.
    ///
    /// NOTE: `body` must not run inside the render itself. Dioxus catches a
    /// panic in a component's render and turns it into an error for the
    /// nearest boundary, so a failed assertion there prints its message and
    /// the test still passes. An earlier version of this helper did exactly
    /// that, which a deliberately broken preparation showed.
    fn with_controller(body: fn(TunnelController)) {
        std::thread_local! {
            static CONTROLLER: std::cell::Cell<Option<TunnelController>> =
                const { std::cell::Cell::new(None) };
        }

        fn probe() -> Element {
            let warning = use_confirm_slot();
            use_hook(|| {
                let controller = TunnelController::new(
                    warning,
                    SharedPreferences(Signal::new(api::Preferences::default())),
                );
                CONTROLLER.with(|cell| cell.set(Some(controller)));
            });
            rsx! {}
        }

        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        let controller = CONTROLLER
            .with(std::cell::Cell::take)
            .expect("the probe mounted its controller");
        dom.in_scope(dioxus::core::ScopeId::APP, || body(controller));
    }

    /// Host/view retirement during an accepted save must keep the original
    /// warning's action claim. A second intent cannot replace that warning,
    /// and the original intent must be stale when the save finally settles.
    #[test]
    fn source_retirement_cannot_replace_a_warning_with_an_unfinished_save() {
        with_controller(|controller| retire_during_save(controller, true));
        with_controller(|controller| retire_during_save(controller, false));
    }

    /// One case of the test above: the terminal view leaves (`leave`) or is
    /// superseded by another session's view while the accepted save runs.
    fn retire_during_save(mut controller: TunnelController, leave: bool) {
        let generation = controller.enter_view("old-session");
        let source = Source::Terminal {
            session: "old-session".into(),
            generation,
        };
        let (result, mut pending) = oneshot::channel();
        let shown = controller.warning.open(source.clone(), result);
        controller.busy.set(true);
        assert_eq!(controller.accept_warning(&shown), Some(source.clone()));
        // Model a Cancel already queued when Allow disabled the
        // controls; the live claim must win over stale DOM state.
        controller.finish_warning(&shown, Ok(false));
        if leave {
            controller.leave_view(generation);
        } else {
            controller.enter_view("new-session");
        }
        assert_eq!(
            pending.try_recv().unwrap(),
            None,
            "retirement must not release the accepted warning"
        );
        assert!(*controller.busy.peek());
        let mut competing = Box::pin(controller.prepare("unused", Source::Settings));
        let mut context = Context::from_waker(Waker::noop());
        assert!(
            matches!(competing.as_mut().poll(&mut context), Poll::Ready(Err(_))),
            "an unfinished save must exclude a second intent"
        );
        drop(competing);
        assert!(controller.warning.is_current(&shown));

        // This is the actual callback's completion path. The HTTP
        // result is controlled here at its unresolved-save boundary.
        controller.finish_save(&shown, Ok(true));
        assert_eq!(pending.try_recv().unwrap(), Some(Ok(true)));
        assert!(controller.warning.peek_key().is_none());
        assert!(!*controller.saving.peek());
        let mut current = Box::pin(controller.current("unused", &source));
        assert!(
            matches!(current.as_mut().poll(&mut context), Poll::Ready(Ok(false))),
            "the retired terminal must not continue after a successful save"
        );
        drop(current);
        controller.complete();
        assert!(
            !*controller.busy.peek(),
            "completion must leave later intents usable"
        );
    }

    /// Spec (`accept_warning`, `finish_save`): an Allow acts only for the
    /// showing its render drew. Drawn for a warning since dismissed, it
    /// starts no save, does not mark the dialog saving, and leaves the
    /// warning now open (another source's) as it is. The current showing's
    /// Allow completes the source that showing was opened for, a second
    /// Allow from the same burst is refused while the save runs, and the
    /// save's outcome goes to that opening's own preparation.
    ///
    /// Why: the warning text is the same for every source, so a click drawn
    /// for one terminal link's warning that read the open warning at click
    /// time, as this handler once did, would write consent and let a
    /// different link, card or Settings action proceed that the user never
    /// answered.
    #[farhelm_testtrace::test]
    fn a_stale_allow_lets_no_other_source_proceed() {
        with_controller(|mut controller| {
            let generation = controller.enter_view("first-session");
            let first_source = Source::Terminal {
                session: "first-session".into(),
                generation,
            };
            let (first_result, mut first) = oneshot::channel();
            let drawn = controller.warning.open(first_source, first_result);
            controller.finish_warning(&drawn, Ok(false));
            assert_eq!(first.try_recv().unwrap(), Some(Ok(false)));

            let (second_result, mut second) = oneshot::channel();
            let open = controller.warning.open(Source::Settings, second_result);
            assert_eq!(controller.accept_warning(&drawn), None);
            // Checked right after the refusal: the old handler marked the
            // dialog saving before it looked at the warning and cleared the
            // mark only when its spawned task finished, so "never claimed"
            // shows here as `false`, where "claimed and later released" would
            // still read `true`.
            assert!(!*controller.saving.peek(), "a stale Allow claims no save");
            assert!(
                controller.warning.is_current(&open),
                "the open warning stays"
            );
            assert_eq!(second.try_recv().unwrap(), None);

            assert_eq!(controller.accept_warning(&open), Some(Source::Settings));
            assert_eq!(
                controller.accept_warning(&open),
                None,
                "a second Allow starts no second save"
            );
            assert!(
                controller.warning.is_current(&open),
                "the dialog stays up through the save"
            );
            controller.finish_save(&open, Ok(true));
            assert_eq!(second.try_recv().unwrap(), Some(Ok(true)));
            assert!(controller.warning.peek_key().is_none());
            assert!(!*controller.saving.peek());
        });
    }

    /// Spec (`finish_warning`, `retire_warning`): Cancel and Escape close the
    /// warning only for the showing they were drawn for; one drawn for a
    /// showing that reconciliation already retired leaves the next warning
    /// open with its preparation still waiting. Retirement drops the retired
    /// opening's result sender; how a real preparation reads that drop is
    /// `a_retired_warning_answers_its_action_not_allowed`'s job.
    ///
    /// Why: a dismissal that read the open warning at click time would
    /// cancel, and pull focus out of, a warning for another action the user
    /// has not seen yet.
    #[farhelm_testtrace::test]
    fn a_stale_dismissal_leaves_the_open_warning() {
        with_controller(|mut controller| {
            let generation = controller.enter_view("left-session");
            let (first_result, mut first) = oneshot::channel();
            let drawn = controller.warning.open(
                Source::Terminal {
                    session: "left-session".into(),
                    generation,
                },
                first_result,
            );
            controller.leave_view(generation);
            assert!(
                first.try_recv().is_err(),
                "retirement drops the retired opening's sender"
            );

            let (second_result, mut second) = oneshot::channel();
            let open = controller.warning.open(Source::Settings, second_result);
            controller.finish_warning(&drawn, Ok(false));
            assert!(
                controller.warning.is_current(&open),
                "the open warning stays"
            );
            assert_eq!(second.try_recv().unwrap(), None);

            controller.finish_warning(&open, Ok(false));
            assert_eq!(second.try_recv().unwrap(), Some(Ok(false)));
            assert!(controller.warning.peek_key().is_none());
        });
    }

    /// Spec (`retire_warning`, `prepare`): retiring the open warning ends the
    /// action waiting on it with `Ok(false)`, "not allowed" and no error, and
    /// releases the controller so the next tunnel action is admitted.
    ///
    /// Why: retirement has no token to answer with, so it does not send a
    /// result; it drops the opening's sender, and only the preparation's
    /// `unwrap_or(Ok(false))` turns that into "not allowed". If a cancelled
    /// channel ever mapped to an error instead, every retired warning would
    /// raise an error notice, and if it never resolved, the controller would
    /// stay claimed and refuse every later tunnel action.
    #[farhelm_testtrace::test]
    fn a_retired_warning_answers_its_action_not_allowed() {
        with_controller(|controller| {
            // The Settings source is used because its preparation makes no
            // request and reaches the warning on the first poll; the test
            // runs no helm. Real callers never retire a Settings warning, so
            // this exercises the waiter's reading of a dropped sender, not
            // which sources reconciliation retires.
            let mut context = Context::from_waker(Waker::noop());
            let mut waiting = Box::pin(controller.prepare("unused", Source::Settings));
            assert!(
                waiting.as_mut().poll(&mut context).is_pending(),
                "the action waits on the warning"
            );
            assert_eq!(controller.warning.peek_key(), Some(Source::Settings));
            assert!(*controller.busy.peek());

            controller.retire_warning();
            assert!(controller.warning.peek_key().is_none());
            assert!(
                matches!(waiting.as_mut().poll(&mut context), Poll::Ready(Ok(false))),
                "the waiting action reads retirement as not allowed"
            );
            drop(waiting);
            assert!(
                !*controller.busy.peek(),
                "the retired action releases its claim"
            );

            // A still-claimed controller answers this with
            // `Poll::Ready(Err(_))` ("another tunnel action is still
            // waiting"); an admitted action reaches its own warning instead.
            let mut next = Box::pin(controller.prepare("unused", Source::Settings));
            assert!(
                next.as_mut().poll(&mut context).is_pending(),
                "the next action is admitted"
            );
            assert_eq!(controller.warning.peek_key(), Some(Source::Settings));
        });
    }
}
