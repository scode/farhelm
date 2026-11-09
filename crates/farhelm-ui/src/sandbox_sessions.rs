//! Session-owned sandboxes adapt existing host operations. Ownership is a
//! resource fact, never a claim that the cache lists every session on it.

use crate::{Host, HostId, HostPhase, Session};
use dioxus::prelude::*;
use farhelm_proto::tensorlake::{Settings, SshKeyStatus, recipes::RecipeView};

pub(crate) const DELETE_CONSEQUENCE: &str = "Delete the whole sandbox? Every session, running process, file, checkout, unpushed change and snapshot in it will be permanently removed. This cannot be undone.";
pub(crate) const STOP_BILLING: &str =
    "The sandbox keeps running and billing until you suspend or delete it.";

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
    hosts
        .iter()
        .find(|host| Some(host.id) == session.host && host.is_session_owned)
}

/// A display name describes the recipe; operations still use the host ID.
pub(crate) fn label(host: &Host) -> String {
    host.tensorlake
        .as_ref()
        .map(|binding| format!("{} sandbox", binding.recipe.raw()))
        .unwrap_or_else(|| "Session sandbox".into())
}

/// Both suspended and changing resources withhold supervisor lifecycle verbs.
pub(crate) fn frozen(host: &Host) -> bool {
    matches!(
        host.state,
        HostPhase::Suspended { .. } | HostPhase::TensorlakeChanging { .. }
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
        notice.explanation = vec![DetailPart::text(
            "This session's sandbox is suspended. Its processes are preserved, including the agent's running command, and continue when you resume it. Only snapshot storage bills while suspended.",
        )];
        notice.detail = vec![DetailPart::text(match suspended_by_farhelm_at {
            Some(at) => format!(
                "Suspended from Farhelm at {}",
                crate::activity::utc_stamp(at / 1000)
            ),
            None => format!(
                "Suspended outside Farhelm, noticed at {}. Tensorlake suspends this sandbox after 30 minutes without a connection from Farhelm.",
                crate::activity::utc_stamp(observed_at / 1000)
            ),
        })];
        if let Some(error) = resume_failed {
            notice.detail.extend([
                DetailPart::text(" Resume failed: "),
                DetailPart::peer(error),
            ]);
        }
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
enum Operation {
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
    async fn execute(
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
/// The caller supplies its existing page claim, so header and row actions
/// cannot mutate under one another while the activity read or request waits.
#[component]
pub(crate) fn Controls(
    host: Host,
    session: Session,
    disabled: bool,
    claim: Callback<(), Option<crate::ops::OpGuard>>,
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
    let mut held = use_signal(|| None::<crate::ops::OpGuard>);
    let suspended = matches!(host.state, HostPhase::Suspended { .. });
    let changing = matches!(host.state, HostPhase::TensorlakeChanging { .. });
    let host_id = host.id;
    let execute_base = base.clone();
    let execute_session = session.clone();
    let execute = use_callback(move |operation: Operation| {
        question.set(None);
        busy.set(true);
        let base = execute_base.clone();
        let session = execute_session.clone();
        spawn(async move {
            let result = operation
                .execute(
                    &base,
                    host_id,
                    session.id.raw(),
                    session.tabs.is_empty(),
                    session.status.has_ended(),
                )
                .await;
            error.set(result.err());
            busy.set(false);
            held.set(None);
            on_done.call(());
        });
    });
    let stop_asks = !skip && !session.tabs.is_empty();
    let start = use_callback(move |stop: bool| {
        if disabled || changing || *busy.peek() || question.peek().is_some() {
            return;
        }
        let Some(guard) = claim.call(()) else {
            return;
        };
        held.set(Some(guard));
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
    rsx! {
        span { class: "sandbox-session-controls",
            if let Some(operation) = question() {
                span { class: "sandbox-session-question header-confirm", role: "alertdialog", tabindex: "-1",
                    aria_label: if operation == Operation::StopAndSuspend { "Stop and suspend sandbox?" } else { "Suspend sandbox?" },
                    onmounted: move |_| crate::focus_confirmation(".sandbox-session-question .btn-danger", ".sandbox-session-question .btn-neutral"),
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.prevent_default(); event.stop_propagation(); question.set(None); held.set(None);
                        }
                    },
                span { class: "confirm-consequence",
                    if operation == Operation::StopAndSuspend {
                        "Stop this session's agent and close its terminal tabs, then suspend the whole sandbox? Every other process freezes and continues on Resume. The stopped agent and closed tabs do not continue. "
                    } else {
                        "Suspend the whole sandbox? Every process in it freezes, including any other sessions, and continues on Resume. "
                    }
                    "Files remain; snapshot storage still bills."
                }
                button { r#type: "button", class: "btn btn-neutral",
                    "data-tooltip": if operation == Operation::StopAndSuspend { "Cancel: keep the agent, its terminal tabs and the sandbox running" } else { "Cancel: keep the sandbox running" },
                    onclick: move |_| { question.set(None); held.set(None); }, "Cancel" }
                button { r#type: "button", class: "btn btn-danger", onkeydown: crate::guard_confirmation_enter,
                    "data-tooltip": if operation == Operation::StopAndSuspend { "Stop this session's agent and close its terminal tabs, then suspend the whole sandbox now" } else { "Suspend the whole sandbox now, freezing every process in it until Resume" },
                    onclick: move |_| execute.call(operation), if operation == Operation::StopAndSuspend { "Stop and suspend" } else { "Suspend" } }
                }
            } else if let Some(buttons) = buttons {
                {buttons.call(ControlButtons {
                    resume: suspended, disabled: disabled || busy() || changing, start,
                })}
            } else {
                button { r#type: "button", class: if suspended { "btn btn-primary" } else { "btn btn-neutral" },
                    disabled: disabled || busy() || changing,
                    "data-tooltip": if suspended { "Resume: start this session's sandbox again; its frozen processes continue" } else { "Suspend: freeze every process in this session's sandbox until Resume; files remain and snapshot storage still bills" },
                    onclick: move |_| start.call(false),
                    crate::icons::SandboxPause { resume: suspended }
                    if suspended { "Resume" } else { "Suspend" }
                }
                if stop_choice && !suspended {
                    button { r#type: "button", class: "btn btn-danger", disabled: disabled || busy() || changing,
                        "data-tooltip": "Stop and suspend: stop this session's agent and close its terminal tabs, then suspend the whole sandbox",
                        onclick: move |_| start.call(true), "Stop and suspend" }
                }
            }
            if let Some(error) = error() { span { role: "alert", "{crate::peer::display_peer(&error)}" } }
        }
    }
}

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
