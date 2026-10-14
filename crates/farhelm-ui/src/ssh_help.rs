//! One read-only dialog for the help menu and the still-mounted add-host form.
//! The helm composes commands; this component only presents and copies them.

use crate::{
    ApiBase,
    api::{SshHostCommands, fetch_ssh_connection_help},
    hosts::settings_dialog::{HostDialogHeader, install_dialog_with_selector},
    modal_isolation,
    peer::PeerLine,
};
use dioxus::prelude::*;

const SELECTOR: &str = r#".ssh-help-dialog[role="dialog"]"#;

/// Release the child modal before returning focus to its opener. Add host
/// remains mounted with its draft intact; it must regain isolation too.
pub(crate) fn return_focus(add_host: bool) {
    let parent = if add_host {
        modal_isolation::install_js(
            r#".host-add-dialog[role="dialog"]"#,
            Some(".add-host-cancel"),
        )
    } else {
        String::new()
    };
    let target = if add_host {
        ".add-host-ssh-help"
    } else {
        ".app-help-toggle"
    };
    document::eval(&format!(
        "{} requestAnimationFrame(() => {{ document.querySelector('{target}')?.focus({{preventScroll:true}}); {parent} }});",
        modal_isolation::release_js(SELECTOR)
    ));
}

/// Fetch once per opening, never block dismissal on HTTP, and preserve an
/// unregistered destination as a snapshot of the add form's draft.
#[component]
pub(crate) fn SshHelpDialog(destination: Option<String>, on_close: EventHandler<()>) -> Element {
    let base = use_context::<ApiBase>().0;
    let extra_requested = destination.is_some();
    let mut result = use_signal(|| None);
    use_hook(move || {
        spawn(async move {
            result.set(Some(
                fetch_ssh_connection_help(&base, destination.as_deref()).await,
            ));
        })
    });
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div { class: "app-settings-dialog ssh-help-dialog host-dialog-framed", role: "dialog",
                aria_modal: "true", aria_label: "ssh needs 2FA?", tabindex: "-1",
                onmounted: move |_| install_dialog_with_selector(SELECTOR, ".ssh-help-close", Some(".ssh-help-close")),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape && !event.is_composing() { on_close.call(()); }
                },
                HostDialogHeader {
                    title: rsx! { "ssh needs 2FA?" }, summary: "start a host's ssh connections yourself",
                    dismiss_label: "close ssh help", dismiss_tooltip: "close: return without adding a host or starting ssh",
                    on_dismiss: move |_| on_close.call(()),
                }
                div { class: "host-settings-body ssh-help-body",
                    p {
                        "Farhelm connects without prompting, so it cannot answer a second factor or other approval on every ssh login. Run both commands in a terminal "
                        if cfg!(all(native_desktop, target_os = "macos")) { strong { "on this Mac" } }
                        else { strong { "on the machine the helm runs on, as the user it runs as" } }
                        ". Each asks for approval once, then keeps a connection open in the background for Farhelm. When a connection drops, run its command again."
                    }
                    p { "These commands disable agent, X11 and port forwarding, as Farhelm does. Port forwards belong to the master connection and cannot be cancelled by clients that reuse it." }
                    p { "We recommend taking a screenshot of this dialog and asking an agent to set up a script that automates these commands for you." }
                    match result.read().as_ref() {
                        None => rsx! { p { role: "status", "loading connection commands…" } },
                        Some(Err(error)) => rsx! { PeerLine { class: "create-session-error", parts: vec![crate::peer::DetailPart::Peer(error.clone())] } },
                        Some(Ok(help)) => rsx! {
                            for reason in &help.unavailable { PeerLine { class: "create-session-error", parts: vec![crate::peer::DetailPart::Peer(reason.clone())] } }
                            if extra_requested {
                                if let Some(extra) = &help.extra { HostCommands { host: extra.clone() } }
                                else { section { h3 { "the host you're adding" } p { "Enter an ssh destination in Add host first." } } }
                            }
                            for host in &help.hosts { HostCommands { host: host.clone() } }
                            if help.hosts.is_empty() { p { "No ssh hosts are registered. Use add host in the hosts panel to register one." } }
                        },
                    }
                }
                div { class: "host-settings-footer",
                    button { r#type: "button", class: "btn btn-neutral ssh-help-close", "data-tooltip": "close: return without starting ssh",
                        onclick: move |_| on_close.call(()), "close" }
                }
            }
        }
    }
}

/// Commands stay literal in the clipboard while peer text is escaped on screen.
#[component]
fn HostCommands(host: SshHostCommands) -> Element {
    rsx! {
        section { class: "ssh-help-host host-settings-section",
            h3 { dir: "ltr", "{crate::peer::display_peer(&host.name)}" }
            p { class: "peer-value", dir: "ltr", "{crate::peer::display_peer(&host.destination)}" }
            for (label, command) in [("sessions connection", host.sessions), ("setup and updates connection", host.setup)] {
                if let Some(command) = command {
                    div { class: "ssh-help-command",
                        strong { "{label}" }
                        pre { dir: "ltr", "{crate::peer::display_peer(&command)}" }
                        button { r#type: "button", class: "btn btn-neutral", aria_label: "copy {label}",
                            "data-tooltip": "copy: the exact ssh command to run on the helm's machine",
                            onclick: move |_| crate::session_view::copy_to_clipboard(&command), "copy" }
                    }
                }
            }
        }
    }
}
