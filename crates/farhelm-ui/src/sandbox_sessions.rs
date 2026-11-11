//! Session-owned environments adapt existing host operations. Ownership is a
//! resource fact, never a claim that the cache lists every session on it.

use crate::{Host, HostId, HostPhase, Session};
use dioxus::prelude::*;
use farhelm_proto::tensorlake::{Settings, SshKeyStatus, recipes::RecipeView};

/// A recipe is authorable without these launch prerequisites. Registration
/// cannot be checked by the saved API key and is deliberately absent here.
pub(crate) fn recipe_refusal(recipe: &RecipeView, settings: &Settings) -> Option<String> {
    if !settings.api_key_set {
        return Some("Set a Tensorlake API key in Settings".into());
    }
    match &settings.ssh_key {
        SshKeyStatus::NotCreated => {
            return Some("Create Farhelm's SSH key in Settings → Tensorlake".into());
        }
        SshKeyStatus::Unavailable { reason } => {
            return Some(format!("Farhelm's local SSH key is unavailable: {reason}"));
        }
        SshKeyStatus::Ready { .. } => {}
    }
    recipe.snapshot.is_none().then(|| "No snapshot yet".into())
}

/// The picker keeps unavailable recipes inspectable, with their own reason.
#[derive(Clone, PartialEq)]
pub(crate) struct RecipeChoice {
    pub(crate) name: String,
    pub(crate) size: String,
    pub(crate) refusal: Option<String>,
}

/// Resolve ownership only from host metadata, never a session title or name.
pub(crate) fn owned_host<'a>(session: &Session, hosts: &'a [Host]) -> Option<&'a Host> {
    hosts.iter().find(|host| {
        Some(host.id) == session.host
            && host.is_session_owned
            && host.kind.environment_provider().is_some()
    })
}

/// A display name describes the recipe; operations still use the host ID.
pub(crate) fn label(host: &Host) -> String {
    if let Some(environment) = &host.environment {
        let places = environment
            .target
            .place()
            .host()
            .map(|_| farhelm_proto::EnvironmentPlaceView {
                place: environment.target.place(),
                name: environment
                    .place_name
                    .clone()
                    .unwrap_or_else(|| "unavailable host".into()),
                reason: None,
                refreshing: false,
            })
            .into_iter()
            .collect::<Vec<_>>();
        return crate::environment_ui::target_label(&environment.target, &places);
    }
    host.tensorlake
        .as_ref()
        .map(|binding| format!("{} sandbox", binding.recipe.raw()))
        .unwrap_or_else(|| "Session sandbox".into())
}

/// Old public host replies keep the resource name in their Tensorlake field.
/// Confirmation can preserve that identity while newer replies carry shared facts.
pub(crate) fn resource_name(host: &Host) -> Option<&str> {
    host.environment
        .as_ref()
        .map(|environment| environment.name.as_str())
        .or_else(|| {
            host.tensorlake
                .as_ref()
                .map(|binding| binding.sandbox.raw())
        })
}

/// Only known environment kinds reach owned controls; unknown future kinds
/// retain ordinary diagnostics without verbs this client cannot vouch for.
pub(crate) fn provider_ui(host: &Host) -> crate::environment_ui::ProviderUi {
    crate::environment_ui::ProviderUi(
        host.kind
            .environment_provider()
            .expect("owned_host admitted a known environment kind"),
    )
}

/// Both suspended and changing resources withhold supervisor lifecycle verbs.
pub(crate) fn frozen(host: &Host) -> bool {
    matches!(
        host.state,
        HostPhase::Suspended { .. }
            | HostPhase::Stopped { .. }
            | HostPhase::TensorlakeChanging { .. }
    )
}

/// Keep ordinary transition details, adapting a settled suspension's fixed
/// timeout and process-preservation wording for session-owned resources.
pub(crate) fn unavailable_notice(host: &Host) -> crate::hosts::SessionUnavailableNotice {
    use crate::peer::DetailPart;
    let mut notice =
        crate::hosts::stale_session_notice(&label(host), crate::hosts::HostLookup::Known(host));
    if let HostPhase::Suspended {
        observed_at,
        suspended_by_farhelm_at,
        resume_failed,
    } = &host.state
    {
        notice.cause = "Suspended";
        let presentation = provider_ui(host);
        notice.explanation = vec![DetailPart::text(presentation.suspended_explanation())];
        notice.detail = presentation.suspended_detail(
            *observed_at,
            *suspended_by_farhelm_at,
            resume_failed.as_ref(),
            true,
        );
    }
    if matches!(
        host.state,
        HostPhase::TensorlakeChanging { starting: true, .. }
    ) {
        notice.cause = "Starting environment";
        notice.explanation = vec![DetailPart::text(provider_ui(host).starting_help())];
    }
    notice
}

/// Share the ordinary host activity rule. A successful empty cache on an
/// offline host is not evidence that suspension will leave no working agent.
pub(crate) async fn suspend_asks(
    base: &str,
    host: HostId,
    hosts: Signal<crate::hosts::HostsRead>,
) -> bool {
    let filter = crate::api::SessionFilter {
        host: Some(host),
        ..Default::default()
    };
    let listing = crate::api::fetch_sessions(base, &filter, crate::api::ListSort::default()).await;
    // Read through the same refresh owner as Hosts. A failed refresh's retained
    // snapshot cannot authorize immediate Suspend, even after an empty listing.
    let connected = matches!(hosts.peek().lookup(Some(host)),
        crate::hosts::HostLookup::Known(current) if crate::hosts::is_connected(&current.state));
    !connected
        || !listing.is_ok_and(|listing| {
            !listing.truncated
                && listing.sessions.iter().all(|session| {
                    !crate::hosts::suspend_status_needs_confirmation(&session.status, session.stale)
                })
        })
}

/// Consent names an operation, not a toggle against the host's latest phase.
/// Another viewer may suspend the resource while this viewer's question is
/// open; answering Suspend must never turn that external change into Resume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Suspend,
    Resume,
    StopAndSuspend,
}

impl Operation {
    /// Freeze the clicked button's direction before activity reads can yield.
    fn requested(stop: bool, suspended: bool) -> Self {
        if stop {
            Self::StopAndSuspend
        } else if suspended {
            Self::Resume
        } else {
            Self::Suspend
        }
    }

    /// Dispatch captured consent without consulting a refreshed host phase.
    /// Stop's guards still cover the selected session; the host endpoint owns
    /// current resource admission and reports incompatible state normally.
    pub(crate) async fn execute(
        self,
        base: &str,
        host: HostId,
        session: &str,
        no_tabs: bool,
        ended: bool,
    ) -> Result<(), String> {
        match self {
            Self::Suspend => crate::api::suspend_host(base, host).await,
            Self::Resume => crate::api::resume_host(base, host).await,
            Self::StopAndSuspend => {
                crate::api::stop_and_suspend(base, session, no_tabs, ended).await
            }
        }
    }
}

/// Reuse the existing host lifecycle endpoints for a session-facing control.
/// The caller supplies its session claim, held during activity reads and
/// consent, then transferred to ListView before an accepted request can yield.
/// Navigation can discard this surface without cancelling the request.
///
/// Suspend and Resume are not host registry changes, so this takes no page
/// token: launches on other hosts and host changes elsewhere proceed beside
/// it. `ops` is read only for this page's launch target. While a launch
/// targets or wakes this very host, the buttons are disabled and a click or
/// answer is refused; once the request is sent, a launch on this host is left
/// to the launcher's own availability rule and the helm's lifecycle lock.
#[component]
pub(crate) fn Controls(
    host: Host,
    session: Session,
    disabled: bool,
    ops: crate::ops::OpLock,
    claim: Callback<(), Option<crate::session_ops::SessionGuard>>,
    #[props(default)] stop_choice: bool,
    #[props(default)] buttons: Option<Callback<ControlButtons, Element>>,
    on_done: EventHandler<()>,
) -> Element {
    let base = use_context::<crate::ApiBase>().0;
    let skip = use_context::<crate::list::SharedPreferences>()
        .0
        .read()
        .skip_session_confirmations
        == Some(true);
    let mut busy = use_signal(|| false);
    let hosts = use_context::<Signal<crate::hosts::HostsRead>>();
    let mut error = use_signal(|| None::<String>);
    let mut question = use_signal(|| None::<Operation>);
    let mut held = use_signal(|| None::<crate::session_ops::SessionGuard>);
    // Stop's no-tabs and ended-agent guards as the clicked render showed them
    // (SPEC.md "Lifecycle operations"). The listing can refresh during the
    // activity read, and `execute` then runs with the newer render's session.
    // Every click sets it; the default refuses the most.
    let mut stop_guards = use_signal(|| (true, true));
    let operations = use_context::<crate::session_ops::SessionOps>();
    let suspended = matches!(
        host.state,
        HostPhase::Suspended { .. }
            | HostPhase::Stopped {
                waiting_for_place: false,
                ..
            }
    );
    let changing = matches!(
        host.state,
        HostPhase::TensorlakeChanging { .. }
            | HostPhase::Stopped {
                waiting_for_place: true,
                ..
            }
    );
    let stopped = matches!(host.state, HostPhase::Stopped { .. });
    let presentation = provider_ui(&host);
    let host_id = host.id;
    let suspend_help = provider_ui(&host).suspend_help();
    let execute_session = session.clone();
    let execute = use_callback(move |operation: Operation| {
        question.set(None);
        // The activity read and the question can outlast the click's check.
        // A launch that began targeting this host meanwhile wins; say so,
        // since nothing on screen would otherwise explain the lost answer.
        if ops.launch_targets_now(host_id) {
            held.set(None);
            busy.set(false);
            error.set(Some(LAUNCH_ON_THIS_HOST.into()));
            return;
        }
        let session = execute_session.clone();
        let guard = held.write().take();
        if let Some(guard) = guard {
            if operation == Operation::StopAndSuspend {
                let (no_tabs, ended) = *stop_guards.peek();
                operations.send(crate::session_ops::HeaderAction::Stop(
                    session, no_tabs, ended, true, guard,
                ));
            } else {
                operations.send(crate::session_ops::HeaderAction::Sandbox(
                    session, host_id, operation, guard,
                ));
            }
        }
        busy.set(false);
        on_done.call(());
    });
    let clicked_guards = (session.tabs.is_empty(), session.status.has_ended());
    let stop_asks = !skip && !clicked_guards.0;
    let start = use_callback(move |stop: bool| {
        if disabled
            || changing
            || *busy.peek()
            || question.peek().is_some()
            || ops.launch_targets_now(host_id)
        {
            return;
        }
        let Some(guard) = claim.call(()) else {
            return;
        };
        held.set(Some(guard));
        stop_guards.set(clicked_guards);
        error.set(None);
        let operation = Operation::requested(stop, suspended);
        if operation == Operation::Resume {
            execute.call(operation);
            return;
        }
        busy.set(true);
        let base = base.clone();
        spawn(async move {
            let ask = suspend_asks(&base, host_id, hosts).await || (stop && stop_asks);
            busy.set(false);
            if ask {
                question.set(Some(operation));
            } else {
                execute.call(operation);
            }
        });
    });
    let launch_here = ops.launch_targets(host_id);
    rsx! {
        span { class: "sandbox-session-controls",
            if let Some(operation) = question() {
                span { class: "sandbox-session-question header-confirm", role: "alertdialog", tabindex: "-1",
                    aria_label: presentation.suspend_title(operation == Operation::StopAndSuspend),
                    onmounted: move |_| {
                        if operation == Operation::StopAndSuspend {
                            // Activity is read asynchronously. This Stop question
                            // must not take input from a session opened meanwhile.
                            let focus = crate::modal_dialog::FocusTarget::new(
                                ".sandbox-session-question .btn-danger:not(:disabled)",
                                Some(".sandbox-session-question .btn-neutral"),
                                false,
                            ).async_script(None);
                            document::eval(&format!("requestAnimationFrame(() => {{ {focus} }});"));
                        } else {
                            crate::focus_confirmation(".sandbox-session-question .btn-danger", ".sandbox-session-question .btn-neutral");
                        }
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.prevent_default(); event.stop_propagation(); question.set(None); held.set(None);
                        }
                    },
                span { class: "confirm-consequence",
                    {presentation.suspend_question(operation == Operation::StopAndSuspend)}
                    "{suspend_help}"
                }
                button { r#type: "button", class: "btn btn-neutral",
                    "data-tooltip": presentation.suspend_cancel(operation == Operation::StopAndSuspend),
                    onclick: move |_| { question.set(None); held.set(None); }, "Cancel" }
                button { r#type: "button", class: "btn btn-danger", onkeydown: crate::guard_confirmation_enter,
                    disabled: launch_here,
                    "data-tooltip": presentation.suspend_action(operation == Operation::StopAndSuspend),
                    onclick: move |_| execute.call(operation), if operation == Operation::StopAndSuspend { "Stop and suspend" } else { "Suspend" } }
                }
            } else if let Some(buttons) = buttons {
                {buttons.call(ControlButtons {
                    resume: suspended, disabled: disabled || busy() || changing || launch_here, start,
                })}
            } else {
                button { r#type: "button", class: if suspended { "btn btn-primary" } else { "btn btn-neutral" },
                    disabled: disabled || busy() || changing || launch_here,
                    "data-tooltip": if suspended { presentation.resume_help(stopped) } else { suspend_help },
                    onclick: move |_| start.call(false),
                    crate::icons::SandboxPause { resume: suspended }
                    if suspended { "Resume" } else { "Suspend" }
                }
                if stop_choice && !suspended {
                    button { r#type: "button", class: "btn btn-danger", disabled: disabled || busy() || changing || launch_here,
                        "data-tooltip": presentation.suspend_action(true),
                        onclick: move |_| start.call(true), "Stop and suspend" }
                }
            }
            if let Some(error) = error() { span { role: "alert", "{crate::peer::display_peer(&error)}" } }
        }
    }
}

/// Why a Suspend, Resume or Stop and suspend of a host was not sent: this
/// page's own launch targets or is waking that host.
pub(crate) const LAUNCH_ON_THIS_HOST: &str =
    "A launch on this host is in progress; try again once it finishes";

/// A row supplies its established menu focus wiring without owning lifecycle
/// admission or confirmation. Header controls use the adapter's default buttons.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ControlButtons {
    pub(crate) resume: bool,
    pub(crate) disabled: bool,
    pub(crate) start: Callback<bool>,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::Operation;
    use std::future::Future;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A question or activity read can outlive the phase that offered its
    /// button. The actual HTTP endpoint must still express the captured action,
    /// even when a refreshed phase would now offer its opposite.
    #[farhelm_testtrace::test]
    async fn captured_suspend_and_resume_keep_their_endpoint_after_a_phase_change() {
        let mut dom = dioxus::prelude::VirtualDom::new(dioxus::core::VNode::empty);
        dom.rebuild_in_place();
        for (initially_suspended, now_suspended, path) in [
            (false, true, "/api/hosts/7/suspend"),
            (true, false, "/api/hosts/7/resume"),
        ] {
            let captured = Operation::requested(false, initially_suspended);
            assert_ne!(captured, Operation::requested(false, now_suspended));
            let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let peer = tokio::spawn(async move {
                tokio::time::timeout(std::time::Duration::from_secs(10), async move {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut request = Vec::new();
                    let mut chunk = [0; 1024];
                    loop {
                        let read = socket.read(&mut chunk).await.unwrap();
                        assert!(read > 0, "the request must finish its headers");
                        request.extend_from_slice(&chunk[..read]);
                        assert!(request.len() <= 8192, "bounded fixture request");
                        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let request = std::str::from_utf8(&request).unwrap();
                    assert!(request.starts_with(&format!("POST {path} HTTP/1.1\r\n")), "{request}");
                    let response = format!("HTTP/1.1 200 OK\r\n{}: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", crate::skew::BUILD_HEADER, crate::skew::CLIENT_BUILD);
                    socket.write_all(response.as_bytes())
                        .await.unwrap();
                }).await.expect("the captured operation must reach its endpoint");
            });
            // The protected HTTP funnel observes the same root-scoped build
            // signals as the app; scope each poll without retaining a runtime
            // guard while Tokio services the owned peer.
            let mut request = Box::pin(captured.execute(&base, 7, "session", true, true));
            std::future::poll_fn(|cx| {
                dom.in_scope(dioxus::core::ScopeId::APP, || request.as_mut().poll(cx))
            })
            .await
            .unwrap();
            peer.await.unwrap();
        }
    }
}
