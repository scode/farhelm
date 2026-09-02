//! The app-level sidebar bar that identifies the builds participating in this page.
//!
//! The helm's reported build is available through the existing skew latch, so
//! this surface stays a read-only view of that signal. It deliberately shows
//! the client build until a reported mismatch gives it a more useful helm
//! value; a page never needs a loading placeholder to identify its bundle.

use dioxus::prelude::*;

use crate::peer::display_peer;
use crate::skew::{self, Skew};

/// Select the version string the sidebar should show for the current skew state.
///
/// `None` means no mismatch has been latched — either no reply has arrived yet,
/// or every reply so far agreed with this client's build, which is the healthy
/// steady state and the common one. In both cases the compiled client build IS
/// the helm's version as far as anything can tell (agreement means the two
/// stamps are the same string), so showing it is exact, not a placeholder. A
/// silent helm is the skew banner's business. A reported stamp identifies the
/// helm that actually answered, and is shown as sent.
fn displayed_version(skew: Option<&Skew>) -> &str {
    match skew {
        Some(Skew::Reported(stamp)) => stamp,
        Some(Skew::Silent) | None => skew::CLIENT_BUILD,
    }
}

/// Render the slim sidebar bar that keeps the client and helm build identity
/// visible while the rest of the authenticated UI changes underneath it.
#[component]
pub(crate) fn AppBar() -> Element {
    let skew = skew::HELM_BUILD_SKEW.read();
    // A reported stamp is text the helm sent, so it goes through the same
    // display boundary every relayed value does (`peer.rs`): invisible and
    // direction-changing characters become visible escapes, and the element
    // is bidi-isolated. The client build takes the same path for uniformity.
    let version = display_peer(displayed_version(skew.as_ref()));

    rsx! {
        div {
            class: "app-bar",
            span {
                class: "app-version peer-value",
                dir: "ltr",
                title: "this client was built as farhelm {skew::CLIENT_BUILD}",
                "{version}"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `None` is both the pre-reply state and the healthy steady state after
    /// agreeing replies; either way the client build is the exact answer, so
    /// there must be no loading placeholder and no wait for the network.
    #[test]
    fn no_skew_shows_the_client_build() {
        assert_eq!(displayed_version(None), skew::CLIENT_BUILD);
    }

    /// A silent helm is already represented by the skew system; the app bar
    /// keeps the client build visible because there is no remote stamp to show.
    #[test]
    fn silent_skew_shows_the_client_build() {
        assert_eq!(displayed_version(Some(&Skew::Silent)), skew::CLIENT_BUILD);
    }

    /// Once the helm reports a different build, the readout must expose that
    /// exact stamp so the user can identify the remote process behind the skew.
    #[test]
    fn reported_skew_shows_the_helm_build() {
        let skew = Skew::Reported("0.9.0-rc.1".to_string());
        assert_eq!(displayed_version(Some(&skew)), "0.9.0-rc.1");
    }
}
