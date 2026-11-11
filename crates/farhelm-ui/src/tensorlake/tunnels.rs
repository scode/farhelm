//! Forwarding health is independent of a connected supervisor: agents may
//! work while their inference endpoint is unreachable, so keep both visible.
use dioxus::prelude::*;
use farhelm_proto::tensorlake::{TunnelState, TunnelView};

/// A connected host with failed or retrying forwards gets the ordinary
/// amber attention color. Startup without a failure does not claim one.
pub(crate) fn failed(host: &crate::Host) -> bool {
    crate::hosts::is_connected(&host.state)
        && host
            .environment
            .as_ref()
            .and_then(|environment| environment.tunnels.as_ref())
            .or_else(|| {
                host.tensorlake
                    .as_ref()
                    .and_then(|sandbox| sandbox.tunnels.as_ref())
            })
            .is_some_and(|view| {
                matches!(
                    &view.state,
                    TunnelState::Failed { .. }
                        | TunnelState::Retrying {
                            reason: Some(_),
                            ..
                        }
                )
            })
}

/// One health sentence and one mapping per configured listener. All values
/// from recipes or SSH are escaped like other host diagnostics.
#[component]
pub(crate) fn TunnelDetails(view: TunnelView) -> Element {
    let status = match &view.state {
        TunnelState::Up => "Tunnels up".to_string(),
        TunnelState::Failed { reason } => {
            format!("Tunnels failed: {}", crate::peer::display_peer(reason))
        }
        TunnelState::Retrying { reason, delay_secs } => match reason {
            Some(reason) => format!(
                "Tunnels failed; retrying in {delay_secs}s: {}",
                crate::peer::display_peer(reason)
            ),
            None => "Tunnels connecting…".to_string(),
        },
    };
    rsx! {
        div { class: "host-tunnel-details", "data-tunnel-state": match view.state { TunnelState::Up => "up", TunnelState::Failed { .. } => "failed", TunnelState::Retrying { .. } => "retrying" },
            p { class: "host-detail", "{status}" }
            for tunnel in view.tunnels {
                p { class: "host-detail", "127.0.0.1:{tunnel.sandbox_port} → {crate::peer::display_peer(&tunnel.host)}:{tunnel.port}" }
            }
        }
    }
}
