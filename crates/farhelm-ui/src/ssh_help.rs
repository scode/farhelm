//! One read-only dialog for the help menu and the still-mounted add-host form.
//! The helm composes commands; this component only presents and copies them.
//! Each owner closes through the shared boundary before removing help, and
//! Add restores its existing isolation after focus returns to the intact form.

use crate::{
    ApiBase,
    api::{SshHostCommands, fetch_ssh_connection_help},
    hosts::settings_dialog::HostDialogHeader,
    modal_dialog::{
        DialogFocus, DialogHandle, ModalDialog, RecoveryDelay, RecoveryFocus, ScrollRegion,
    },
    peer::PeerLine,
};
use dioxus::prelude::*;

const SELECTOR: &str = r#".ssh-help-dialog[role="dialog"]"#;
/// Each surviving owner chooses its Help or hidden Add-form return contract.
pub(crate) const DIALOG: DialogHandle = DialogHandle::new(SELECTOR);

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
            ModalDialog {
                handle: DIALOG,
                focus: DialogFocus::remembered(".ssh-help-close", Some(".ssh-help-close"), false, None),
                class: "app-settings-dialog ssh-help-dialog host-dialog-framed", role: "dialog",
                aria_modal: "true", aria_label: "ssh needs 2FA?", tabindex: "-1",
                on_escape: move |event: KeyboardEvent| {
                    if !event.is_composing() { on_close.call(()); }
                },
                HostDialogHeader {
                    title: rsx! { "ssh needs 2FA?" }, summary: "start a host's ssh connections yourself",
                    dismiss_label: "close ssh help", dismiss_tooltip: "close: return without adding a host or starting ssh",
                    on_dismiss: move |_| on_close.call(()),
                }
                ScrollRegion { class: "host-settings-body ssh-help-body",
                    handle: DIALOG, selector: ".ssh-help-body", name: "ssh connection help",
                    // Command arrival withdraws text-only entry. Close wins
                    // only after lost focus, never over an approval's answer.
                    recovery: RecoveryFocus::new(".ssh-help-close", None, false, RecoveryDelay::OneFrame),
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
                        Some(Err(error)) => rsx! { PeerLine { class: "create-session-error", parts: vec![crate::peer::DetailPart::peer(error.clone())] } },
                        Some(Ok(help)) => rsx! {
                            for reason in &help.unavailable { PeerLine { class: "create-session-error", parts: vec![crate::peer::DetailPart::peer(reason.clone())] } }
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

/// Show the helm's command snapshot without changing its executable spelling.
/// Labels and command text escape peer controls on screen; copying takes the
/// original bytes, including any controls in a quoted path or destination.
#[component]
fn HostCommands(host: SshHostCommands) -> Element {
    rsx! {
        section { class: "ssh-help-host host-settings-section",
            h3 { dir: "ltr", "{host.name.escaped()}" }
            p { class: "peer-value font-mono", dir: "ltr", "{host.destination.escaped()}" }
            for (label, command) in [("sessions connection", host.sessions), ("setup and updates connection", host.setup)] {
                if let Some(command) = command {
                    div { class: "ssh-help-command",
                        strong { "{label}" }
                        pre { dir: "ltr", "{command.escaped()}" }
                        button { r#type: "button", class: "btn btn-neutral", aria_label: "copy {label}",
                            "data-tooltip": "copy: the exact ssh command to run on the helm's machine",
                            onclick: move |_| crate::session_view::copy_to_clipboard(command.raw()), "copy" }
                    }
                }
            }
        }
    }
}
