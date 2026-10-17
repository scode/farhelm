# Outage cards can hide their explanation above the reachable area

Reviewed commit: 7eed523f7af20a67316ae477d129cbad961ffa79

## TLDR

In a short window, or with a sufficiently long host error, the outage card can hide its explanation and recovery
instructions above the window, with no way to scroll them into view.

## Details

Suggested bucket: `other`. Confidence: likely; the missing bounds and upward overflow follow from the CSS, but actual
viewport thresholds and engine geometry were not reproduced. Provenance: separate adversarial reviewer in the trailing
review through afc828776596f2e6639d16d4203ccc8b3d0f5862. Proposed drop: none.

At the reviewed commit, `crates/farhelm-ui/assets/app.css:6554–6574` absolutely fills the terminal area and centers the
card vertically, but gives neither card nor overlay a height bound or scrolling. A card taller than that area extends
above and below it. The upward overflow starts before the scroll origin, so ordinary scrolling cannot recover it. The
document also refuses scrolling at `app.css:492–494`. Host remedies and diagnostic text can grow substantially:
`crates/farhelm-ui/src/hosts.rs:683–750` preserves remediation and errors, and `hosts.rs:929–945` puts them in the card.
`crates/farhelm-ui/src/session_view.rs:3147–3162` renders the host card without a focusable descendant. The change
replaces an in-flow notice with this absolute surface. The browser reconnect card shares the same CSS.

Bound the card to the available area, provide scrolling and keyboard access for text-only cards, and keep oversized
content from being centered before the scroll origin. A focused geometry check should use both a short terminal area and
a long wrapped diagnostic, then verify that the entire explanation can be reached with pointer and keyboard.

Possible cover: none exact. SPEC.md's session-availability passage requires a cause card with the remedy underneath;
SPEC_impl.md's unavailable-session presentation describes that card without accepting inaccessible content. The existing
`pinned-host-dialog-chrome-can-hide-content.md` concerns bounded modal frames, and
`short-main-pane-hides-approval-requests.md` concerns approval-region placement. Neither covers this session-card
mechanism. TODO.md's scroll audit is Near term, not Planned, and concerns existing scroll regions. The filter “Dialog
explanations out of view but reachable by scrolling” excludes content that cannot be reached.

Status at pinned tip: the same unbounded centering remains at `crates/farhelm-ui/assets/app.css:6573–6599`; no fixing
commit was identified. No browser reproduction or runtime tests were run. Surrounding implementations and coverage
sources were read selectively; every patch assigned to this reviewer was read in full.
