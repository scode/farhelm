//! The client↔helm half of SPEC_impl.md's version-and-skew story
//! (PLAN_M6.md item 6's tail).
//!
//! The helm↔supervisor edge has refused mismatched peers at the hello since
//! M1. The other edge is the one nobody was watching: a browser tab holds
//! its bundle for as long as it stays open, and upgrading the helm under it
//! leaves that tab talking a contract it was not built for. Nothing on
//! either side notices — the requests are well-formed, the replies decode
//! or do not, and the user sees features that are missing, fields that read
//! as absent, and refusals with no cause on screen. That is precisely the
//! "silent degradation" SPEC.md's version rule forbids.
//!
//! So every reply this UI reads carries the helm's build stamp, this
//! bundle compares it against its own, and a disagreement puts one line on
//! screen saying to reload. The check rides EVERY reply rather than a
//! dedicated endpoint on purpose: the interesting moment is a helm that was
//! upgraded while the tab sat idle, and the first thing that notices is
//! whatever request the tab happens to make next. "Every reply" is meant
//! literally on both ends — the helm stamps its refusals and its 403s too,
//! since a skewed client's requests failing is exactly when it needs to
//! know — and the one request that does not pass through `api::send` (the
//! attachment upload, which hands a `File` to `fetch` from terminal.js)
//! does its own comparison against the stamp the attachment policy carries.
//!
//! The verdict is LATCHED once seen; [`HELM_BUILD_SKEW`] carries why.
//!
//! ## What is deliberately not done here
//!
//! No blocking, no refusing to render, no automatic reload. A stale bundle
//! mostly works, an automatic reload would discard whatever the user was in
//! the middle of, and this UI has no way to know which of the two is worse
//! for them right now. Telling them, precisely, is the whole intervention.

use dioxus::prelude::*;

use crate::peer::{DetailPart, PeerLine};

/// The build this bundle was compiled from — the workspace version, which
/// is the same number the helm and every supervisor carry (Cargo.toml's
/// `[workspace.package]`, and SPEC_impl.md's "one version number across the
/// workspace").
///
/// Baked in at compile time, which is what makes it a statement about the
/// BUNDLE rather than about the machine serving it: that is the whole
/// distinction the check below is drawn on.
pub(crate) const CLIENT_BUILD: &str = env!("CARGO_PKG_VERSION");

/// The response header the helm stamps on every reply (its own
/// `BUILD_STAMP_HEADER`).
///
/// A header rather than a body field, and that is what makes it cheap
/// enough to be universal: every route already answers, none of them had to
/// grow an envelope, and the routes that answer with a bare `{}` or a
/// `text/plain` refusal carry it exactly like the ones that answer with
/// JSON.
pub(crate) const BUILD_HEADER: &str = "x-farhelm-build";

/// How much of a reported stamp is kept for display.
///
/// A version string is a handful of characters; anything past this is not a
/// version and has no business occupying a line of the user's screen. The
/// value is displayed through [`PeerLine`], so it is already escaped and
/// direction-isolated — this bound is about LENGTH, which isolation does
/// nothing for.
const MAX_STAMP_CHARS: usize = 64;

/// What this page has learned about the helm's build, latched at the first
/// DISAGREEMENT and never cleared.
///
/// A global rather than state threaded through the view tree, because the
/// observation happens in `api`'s request functions (every one of them) and
/// the rendering happens once, at the root. Threading a signal through
/// every call site to reach one banner would put the plumbing everywhere
/// and the meaning nowhere.
///
/// The LATCH is the interesting half, and it replaced a plain assignment
/// that was wrong in a way only concurrency makes visible: replies race,
/// and they land in completion order rather than request order, so a slow
/// reply that left the OLD helm could arrive after a fast one from the new
/// build and clear a mismatch that is still true. A verdict cleared by a
/// stale observation is the one failure this surface must not have — it is
/// the only thing telling the user why their session is behaving strangely.
///
/// What the latch costs is a banner that outlives a helm rolled BACK to
/// this bundle's build. That is a real but small cost, and the remedy it
/// keeps recommending stays correct: reloading a page whose bundle already
/// matches is a no-op the user pays once. Certainty about the mismatch is
/// worth more than tidiness about its disappearance.
pub(crate) static HELM_BUILD_SKEW: GlobalSignal<Option<Skew>> = Signal::global(|| None);

/// Whether any reply has yet carried a build stamp at all.
///
/// Separate from the verdict above because it answers a different
/// question: not "do we disagree" but "is the helm on the other end one
/// that speaks this milestone's vocabulary". The terminal heartbeat is
/// gated on it (`reconnect::reconnect_policy`) — a helm that predates the
/// stamp also predates the `ping` message, and would answer it with the
/// silence its own contract prescribes for anything unknown, which the
/// heartbeat would then read as a dead socket and "recover" from forever.
///
/// One-way, like the verdict, and for a simpler reason: a stamp seen once
/// proves the peer's vocabulary, and no later reply can unprove it.
pub(crate) static HELM_BUILD_SEEN: GlobalSignal<bool> = Signal::global(|| false);

/// Whether the helm answering this page is the build this bundle was made
/// for — the browser edge's substitute for a version handshake.
///
/// Both halves are required and they fail differently. A stamp must have
/// been SEEN, because a helm that reports none predates this milestone's
/// vocabulary entirely; and no mismatch may be latched, because a helm on
/// a different build cannot be relied on to honor the fields this UI's
/// unattended behavior depends on — `if_unowned` above all, whose omission
/// turns an automatic reconnect into a session theft.
///
/// Read by `reconnect::reconnect_policy`, which is re-evaluated whenever
/// either signal changes, so a mismatch latched mid-session REVOKES
/// unattended behavior on terminals that are already open rather than only
/// on the next one.
pub(crate) fn helm_is_current() -> bool {
    *HELM_BUILD_SEEN.read() && HELM_BUILD_SKEW.read().is_none()
}

/// What a disagreement is: either a build that differs from this bundle's,
/// or no build reported at all.
///
/// The second case is a mismatch rather than a shrug, which is the opposite
/// of what this module first did. A helm that sends no stamp is a helm from
/// before the stamp existed — which is to say a helm this bundle is genuinely
/// skewed against, and one that does not understand the terminal heartbeat
/// this UI would otherwise send it. "Say nothing" was the comfortable
/// reading; it is also the one that leaves a user with an interface that
/// half-works and no explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Skew {
    /// The helm reported a build, and it is not ours. Carries what it said.
    Reported(String),
    /// The helm reported no build at all.
    Silent,
}

/// Whether `observed` disagrees with `ours`.
///
/// Absent and EMPTY are the same answer — [`Skew::Silent`] — because a
/// header present but blank says exactly as much as one that is missing.
///
/// The comparison is exact rather than semver-aware, deliberately: the
/// question is not "is this compatible" — nothing here can answer that —
/// but "was this page built by the same build that is answering it", and
/// only equality answers that.
fn skew_of(ours: &str, observed: Option<&str>) -> Option<Skew> {
    let Some(observed) = observed.map(str::trim).filter(|value| !value.is_empty()) else {
        return Some(Skew::Silent);
    };
    if observed == ours {
        return None;
    }
    Some(Skew::Reported(
        observed.chars().take(MAX_STAMP_CHARS).collect(),
    ))
}

/// Read one reply's build stamp and record what it says.
///
/// Borrows rather than consuming, and is called for its EFFECT at each call
/// site (`api::send`): a version of this that took and returned the
/// response threaded a side effect through an identity `map`, which reads
/// as a transformation and is not one.
///
/// Only a mismatch is ever recorded, and only the first one (see
/// [`HELM_BUILD_SKEW`]'s latch). Agreement writes nothing at all, which
/// also means the overwhelmingly common case — every reply, forever, on a
/// healthy setup — never marks a signal dirty and never re-renders the tree
/// three times a second.
pub(crate) fn note_build(resp: &reqwest::Response) {
    let observed = resp
        .headers()
        .get(BUILD_HEADER)
        .and_then(|value| value.to_str().ok());
    if observed.is_some_and(|stamp| !stamp.trim().is_empty()) && !*HELM_BUILD_SEEN.peek() {
        *HELM_BUILD_SEEN.write() = true;
    }
    if HELM_BUILD_SKEW.peek().is_some() {
        return;
    }
    if let Some(verdict) = skew_of(CLIENT_BUILD, observed) {
        *HELM_BUILD_SKEW.write() = Some(verdict);
    }
}

/// What the prompt says, as the runs [`PeerLine`] renders.
///
/// It states the FACT, the CONSEQUENCE (this page is the stale one), and
/// the REMEDY (reload) — in that order, because a user who reads only the
/// first clause should still know something real.
///
/// The two shapes say different FACTS, which is why they are not one
/// sentence with a hole in it:
///
/// - [`Skew::Reported`] names BOTH builds, on the same reasoning the host
///   skew chip names both: which side is behind is the first thing anyone
///   asks, and a single number cannot answer it. The helm's stamp is a
///   peer run — a string this UI did not author — so isolating it is what
///   keeps it from rearranging the sentence it sits in.
/// - [`Skew::Silent`] has no version to name and says so in words. Built
///   by interpolation it would leave a blank where a number belongs, which
///   reads as a rendering bug rather than as the diagnosis it is: a helm
///   that reports no build at all is one that predates the reporting.
fn skew_notice(skew: &Skew) -> Vec<DetailPart> {
    let mut parts = vec![
        DetailPart::text("this page was loaded from farhelm "),
        DetailPart::peer(CLIENT_BUILD),
    ];
    match skew {
        Skew::Reported(helm) => {
            parts.push(DetailPart::text(", but the helm now reports "));
            parts.push(DetailPart::peer(helm));
        }
        // No version to name, so the sentence says what IS known rather
        // than leaving a blank where a number should be: a helm that
        // reports no build at all predates the reporting, which is itself
        // the answer to "which side is behind".
        Skew::Silent => parts.push(DetailPart::text(
            ", but the helm is not reporting a build at all — it predates this interface",
        )),
    }
    parts.push(DetailPart::text(
        " — reload to pick up the matching interface (a desktop app needs restarting instead)",
    ));
    parts
}

#[component]
pub(crate) fn BuildSkewNotice() -> Element {
    let skew = HELM_BUILD_SKEW.read().clone();
    rsx! {
        if let Some(skew) = skew {
            PeerLine { class: "build-skew", parts: skew_notice(&skew) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Agreement — the overwhelmingly common case — must say nothing at
    /// all, including when the stamp arrives with the whitespace an HTTP
    /// header can legally carry around its value.
    #[test]
    fn a_matching_stamp_is_silent() {
        assert_eq!(skew_of("0.4.2", Some("0.4.2")), None);
        assert_eq!(skew_of("0.4.2", Some("  0.4.2 ")), None);
    }

    /// A helm that reports NO build is skewed, not silently accepted.
    ///
    /// This is the case the first version of this module got backwards, and
    /// the reasoning that made it look right — "absence is not
    /// disagreement" — is exactly wrong for the only helm that can produce
    /// it. A build with no stamp predates the stamp, which means it also
    /// predates the terminal heartbeat this UI sends and the rest of this
    /// milestone's vocabulary; treating it as agreement leaves a user with
    /// an interface that half-works and nothing on screen about why.
    ///
    /// An empty value is the same answer as no header for the same reason:
    /// a blank stamp says nothing, and "the helm reports ``" is not a
    /// sentence.
    #[test]
    fn a_missing_or_empty_stamp_is_a_mismatch_of_its_own_kind() {
        assert_eq!(skew_of("0.4.2", None), Some(Skew::Silent));
        assert_eq!(skew_of("0.4.2", Some("")), Some(Skew::Silent));
        assert_eq!(skew_of("0.4.2", Some("   ")), Some(Skew::Silent));
    }

    /// A different stamp is reported, and reported as the helm SAID it
    /// rather than as something parsed: the check is deliberately exact
    /// equality, so a build that merely looks newer is still a mismatch.
    #[test]
    fn a_different_stamp_is_reported_verbatim() {
        assert_eq!(
            skew_of("0.4.2", Some("0.5.0")),
            Some(Skew::Reported("0.5.0".to_string()))
        );
        assert_eq!(
            skew_of("0.4.2", Some("0.4.2-dirty")),
            Some(Skew::Reported("0.4.2-dirty".to_string()))
        );
    }

    /// A hostile or broken stamp cannot claim an unbounded amount of the
    /// user's screen. The header comes over a socket, and a value with a
    /// megabyte in it would otherwise be rendered in full on a banner that
    /// sits above the whole app.
    #[test]
    fn an_over_long_stamp_is_cut_down_to_something_displayable() {
        let absurd = "9".repeat(4096);
        let Some(Skew::Reported(reported)) = skew_of("0.4.2", Some(&absurd)) else {
            panic!("an over-long stamp is still a reported mismatch");
        };
        assert_eq!(reported.chars().count(), MAX_STAMP_CHARS);
    }

    /// Both shapes of mismatch must carry fact, consequence, and remedy —
    /// naming what this page was built from, what the helm says (or that it
    /// says nothing), and what to do about it.
    ///
    /// The silent case is the one worth pinning separately: with no version
    /// to print, a notice built by interpolation would leave a blank where a
    /// number belongs, which reads as a rendering bug rather than as the
    /// diagnosis it is.
    #[test]
    fn both_shapes_of_prompt_say_what_happened_and_what_to_do() {
        let flatten = |parts: &[DetailPart]| -> String {
            parts
                .iter()
                .map(|part| match part {
                    DetailPart::Text(text) => text.clone(),
                    DetailPart::Peer(value) => value.clone(),
                })
                .collect()
        };

        let reported = skew_notice(&Skew::Reported("0.5.0".to_string()));
        let peers: Vec<&str> = reported
            .iter()
            .filter_map(|part| match part {
                DetailPart::Peer(value) => Some(value.as_str()),
                DetailPart::Text(_) => None,
            })
            .collect();
        assert_eq!(
            peers,
            vec![CLIENT_BUILD, "0.5.0"],
            "both builds are named, this page's first"
        );
        assert!(flatten(&reported).contains("reload"));

        let silent = flatten(&skew_notice(&Skew::Silent));
        assert!(
            silent.contains("predates this interface"),
            "a helm with no stamp is described rather than left blank: {silent}"
        );
        assert!(silent.contains("reload"), "the remedy is stated either way");
    }
}
