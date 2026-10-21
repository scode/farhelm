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
use crate::peer::display_peer;
use crate::reader::{SurfaceReader, Trigger, request_read};

const DIALOG: &str = ".tunnel-consent-dialog[role=\"dialog\"]";
const SETTINGS: &str = ".app-settings-dialog[role=\"dialog\"]";

/// The action's lifetime follows its source, rather than the selected pane alone.
#[derive(Clone)]
enum Source {
    Terminal { session: String, generation: u64 },
    Card(Box<PendingApproval>),
    Settings,
}

/// A warning owns only its result; the caller retains the intent until acceptance.
struct Warning {
    source: Source,
    result: oneshot::Sender<Result<bool, String>>,
}

/// Page-owned controller, provided above both cards and keyed session views.
#[derive(Clone, Copy)]
pub(crate) struct TunnelController {
    busy: Signal<bool>,
    warning: Signal<Option<Warning>>,
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
    use_context_provider(|| TunnelController {
        busy: Signal::new(false),
        warning: Signal::new(None),
        saving: Signal::new(false),
        view: Signal::new(None),
        next_view: Signal::new(0),
        revision: Signal::new(0),
        notice: Signal::new(None),
        preferences,
    });
}

impl TunnelController {
    /// A newly selected view supersedes earlier terminal intents immediately.
    /// Clear the old view before cancelling its modal, so focus cannot return to it.
    fn enter_view(mut self, session: &str) -> u64 {
        self.view.set(None);
        let terminal_warning = self
            .warning
            .peek()
            .as_ref()
            .is_some_and(|warning| matches!(warning.source, Source::Terminal { .. }));
        // Once Allow starts saving, keep this claim until that save completes.
        // Retiring its source still suppresses the eventual open, but releasing
        // the claim here would let its completion consume a newer warning.
        if terminal_warning && !*self.saving.peek() {
            self.finish_warning(Ok(false));
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
            if !*self.saving.peek() && self.warning.peek().as_ref().is_some_and(|warning|
                matches!(warning.source, Source::Terminal { generation: own, .. } if own == generation))
            {
                self.finish_warning(Ok(false));
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
            self.warning.set(Some(Warning {
                source: source.clone(),
                result: tx,
            }));
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
        let vanished = self.warning.peek().as_ref().is_some_and(|warning| {
            if let Source::Card(card) = &warning.source {
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
        if vanished && !*self.saving.peek() {
            self.finish_warning(Ok(false));
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

    /// The accepted warning retains busy ownership through its save, even if
    /// its source disappears. Preparation rechecks that source before opening.
    fn finish_save(mut self, result: Result<bool, String>) {
        self.saving.set(false);
        self.finish_warning(result);
    }

    /// Close the child modal, restore a still-mounted parent, then return the result.
    fn finish_warning(mut self, result: Result<bool, String>) {
        // A queued Cancel can arrive after Allow starts saving but before DOM
        // disabling renders. Only finish_save may retire that accepted warning.
        if *self.saving.peek() {
            return;
        }
        let Some(warning) = self.warning.write().take() else {
            return;
        };
        let source = warning.source;
        let _ = warning.result.send(result);
        document::eval(&format!("{} requestAnimationFrame(() => {{
            const settings = document.querySelector({settings});
            if (settings) {{ {install} settings.querySelector('.tunnel-settings-switch')?.focus({{preventScroll:true}}); }}
        }});", crate::modal_isolation::release_js(DIALOG),
            settings = serde_json::to_string(SETTINGS).unwrap(),
            install = crate::modal_isolation::install_js(SETTINGS, Some(".app-settings-close"))));
        if matches!(&source, Source::Terminal { session, generation }
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
    let pending = controller.warning.read().is_some();
    rsx! {
        if let Some(error) = controller.notice.read().as_ref() {
            div { class: "tunnel-error-notice", role: "alert",
                span { "{display_peer(error)}" }
                button { r#type: "button", class: "btn btn-neutral", onclick: move |_| controller.notice.set(None), "dismiss" }
            }
        }
        if pending {
            div { class: "host-settings-backdrop tunnel-consent-backdrop", role: "presentation",
                div { class: "tunnel-consent-dialog host-dialog-framed", role: "dialog", aria_modal: "true",
                    aria_labelledby: "tunnel-consent-heading", tabindex: "-1",
                    onmounted: move |_| {
                        document::eval(&crate::modal_isolation::release_js(SETTINGS));
                        crate::hosts::settings_dialog::install_dialog_with_selector(DIALOG, ".tunnel-cancel", Some(".tunnel-cancel"), Some(".tunnel-allow"));
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape && !event.is_composing() && !*controller.saving.peek() {
                            event.stop_propagation(); controller.finish_warning(Ok(false));
                        }
                    },
                    div { class: "host-settings-body",
                        // Saving disables both buttons. The warning's text must
                        // still be reachable in WebKit's keyboard Tab order.
                        tabindex: if (controller.saving)() { Some("0") } else { None },
                        role: if (controller.saving)() { Some("region") } else { None },
                        aria_label: if (controller.saving)() { Some("tunnel security warning") } else { None },
                        h2 { id: "tunnel-consent-heading", "Allow tunnels from remote sessions?" }
                        p { strong { "A remote page opened through a tunnel runs at 127.0.0.1 on this Mac. It can read cookies set by other services on 127.0.0.1; different ports do not keep those cookies separate." } }
                        p { "Your browser treats it as a local page, so its private-network checks do not apply. Any program on this Mac can connect to an open tunnel." }
                        // Consent covers every tunnel, so explain port reuse without
                        // implying this warning names one particular listener.
                        p { "A tunnel uses the remote service's port number on this Mac when it is free, so a Mac program expecting that port may reach the remote host instead." }
                        p { "Ordinary links from other 127.0.0.1 pages into the standalone Farhelm helm are refused." }
                        p { "Allow this only for remote pages you trust. You can turn tunnels off in Settings, which closes every open tunnel." }
                        div { class: "approval-card-actions",
                            button { r#type: "button", class: "btn btn-neutral tunnel-cancel", disabled: (controller.saving)(),
                                onclick: move |_| controller.finish_warning(Ok(false)), "Cancel" }
                            button { r#type: "button", class: "btn btn-primary tunnel-allow", disabled: (controller.saving)(),
                                onclick: move |_| {
                                    if *controller.saving.peek() { return; }
                                    controller.saving.set(true);
                                    let source = controller.warning.peek().as_ref().map(|warning| warning.source.clone());
                                    let base = base.clone();
                                    spawn(async move {
                                        let result = match source {
                                            Some(source) => match controller.current(&base, &source).await {
                                                Ok(true) => api::write_tunnel_consent(&base, true).await.map(|()| {
                                                    controller.preferences.0.write().tunnel_consent = Some(true); true
                                                }),
                                                Ok(false) => Ok(false),
                                                Err(error) => Err(error),
                                            },
                                            None => Ok(false),
                                        };
                                        controller.finish_save(result);
                                    });
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

    /// Host/view retirement during an accepted save must keep the original
    /// warning's action claim. A second intent cannot replace that warning,
    /// and the original intent must be stale when the save finally settles.
    #[test]
    fn source_retirement_cannot_replace_a_warning_with_an_unfinished_save() {
        fn probe() -> Element {
            use_hook(|| {
                for leave in [true, false] {
                    let mut controller = TunnelController {
                        busy: Signal::new(false),
                        warning: Signal::new(None),
                        saving: Signal::new(false),
                        view: Signal::new(None),
                        next_view: Signal::new(0),
                        revision: Signal::new(0),
                        notice: Signal::new(None),
                        preferences: SharedPreferences(Signal::new(api::Preferences::default())),
                    };
                    let generation = controller.enter_view("old-session");
                    let source = Source::Terminal {
                        session: "old-session".into(),
                        generation,
                    };
                    let (result, mut pending) = oneshot::channel();
                    controller.warning.set(Some(Warning {
                        source: source.clone(),
                        result,
                    }));
                    controller.busy.set(true);
                    controller.saving.set(true);
                    // Model a Cancel already queued when Allow disabled the
                    // controls; the live claim must win over stale DOM state.
                    controller.finish_warning(Ok(false));
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
                    assert!(controller.warning.peek().is_some());

                    // This is the actual callback's completion path. The HTTP
                    // result is controlled here at its unresolved-save boundary.
                    controller.finish_save(Ok(true));
                    assert_eq!(pending.try_recv().unwrap(), Some(Ok(true)));
                    assert!(controller.warning.peek().is_none());
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
            });
            rsx! {}
        }
        VirtualDom::new(probe).rebuild_in_place();
    }
}
