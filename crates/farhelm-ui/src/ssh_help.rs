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
/// remains mounted with its draft intact; it must regain isolation too. A probe
/// can replace the opener with a setup offer while help is open, so Cancel is
/// the safe surviving target when that happens.
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
        ".add-host-ssh-help, .host-add-dialog .add-host-cancel"
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
    // Loading, refusals and empty command sets leave this body with only
    // text. Once copy buttons exist they provide its keyboard entry point.
    let has_commands = result
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .is_some_and(|help| {
            help.hosts
                .iter()
                .chain(help.extra.iter().filter(|_| extra_requested))
                .any(|host| host.sessions.is_some() || host.setup.is_some())
        });
    // Command arrival removes the text body's temporary tab stop. Recover
    // focus after that render, before modal isolation has to consume a key.
    // A still-focusable control keeps focus; this is not an opening handoff.
    use_effect(use_reactive((&has_commands,), move |(has_commands,)| {
        if has_commands {
            document::eval(&format!(
                r#"requestAnimationFrame(() => {{
                    const dialog = document.querySelector('{SELECTOR}');
                    if (!dialog) return;
                    const usable = (element) => element instanceof HTMLElement && element.isConnected
                        && dialog.contains(element) && !element.disabled && element.tabIndex >= 0
                        && element.getClientRects().length > 0;
                    if (!usable(document.activeElement))
                        dialog.querySelector('.ssh-help-close')?.focus({{preventScroll: true}});
                }})"#
            ));
        }
    }));
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div { class: "app-settings-dialog ssh-help-dialog host-dialog-framed", role: "dialog",
                aria_modal: "true", aria_label: "ssh needs 2FA?", tabindex: "-1",
                onmounted: move |_| install_dialog_with_selector(SELECTOR, ".ssh-help-close", Some(".ssh-help-close"), None),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape && !event.is_composing() { on_close.call(()); }
                },
                HostDialogHeader {
                    title: rsx! { "ssh needs 2FA?" }, summary: "start a host's ssh connections yourself",
                    dismiss_label: "close ssh help", dismiss_tooltip: "close: return without adding a host or starting ssh",
                    on_dismiss: move |_| on_close.call(()),
                }
                div { class: "host-settings-body ssh-help-body",
                    tabindex: if !has_commands { Some("0") } else { None },
                    role: if !has_commands { Some("region") } else { None },
                    aria_label: if !has_commands { Some("ssh connection help") } else { None },
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
            p { class: "peer-value", dir: "ltr", "{host.destination.escaped()}" }
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
