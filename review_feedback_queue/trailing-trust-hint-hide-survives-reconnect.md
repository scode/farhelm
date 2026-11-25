# Workspace trust hint's × hide survives a reconnect

Reviewed commit: 74c83433cb83c60197eb33939b836bf8afa2b54a

## TLDR

After the user presses × on a workspace trust hint, the app keeps that session's hint hidden until it sees the question
leave the screen. If the connection to the helm drops and the question leaves and comes back while the app is
disconnected (for example the session is restarted from another window and the agent asks again), the app never sees the
gap and keeps the new question's hint hidden. 74c83433 fixed exactly this for the conversation hook hint; the trust hint
was left as it was.

## Details

The trust-band behavior itself predates the range; 74c83433 is the commit whose reasoning applies equally to it.

At the tip, `TrustBand.hidden` (`crates/gui/farhelm-gui/src/session_pane/trust_band.rs:145-151`) is a window-lifetime
`HashSet<(String, TrustAgent)>`, deliberately outside the per-UI-session `Known`; its doc says it is cleared only by a
pushed row showing that agent's question gone. `follow()` (~213-217) keeps entries unless the current row shows the
agent no longer asking; the × handler inserts at ~497. After a reset the new snapshot shows the question again, `retain`
keeps the entry, and `draw` keeps the band hidden. 74c83433's message gives the hook-hint rationale ("could keep an
eligible hint hidden after the app reconnected to the helm") and moves the hook band's hidden set into
`PerSession<Known>` (`hook_band.rs:36-44`). SPEC.md "Managed checkouts" says "× hides only the current occurrence, until
the question leaves the screen"; it does not say whether a reconnect ends an occurrence.

Fix: key the trust band's hidden set per UI session as the hook band now does (accepting that a still-visible occurrence
reappears after reconnect), or record an occurrence identity, which needs protocol support. Verify with a test like
`hook_hint_dismissals_start_over_after_a_ui_reset` for the trust band.

Suggested bucket: `other` (an informational hint stays hidden after a rare disconnect sequence). Confidence: likely. The
code path is established from source. Unverified after investigation: that a question can leave and reappear inside one
disconnect window without a push the app sees (plausible for a restart from another client, not traced through the
helm's reconnect pushes). Caveat from the reviewer: the window-lifetime × was a deliberate port of the old app's
occurrence dismissals, so the maintainer may consider the asymmetry intended.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g01-hint-bands-hooks` record `f2`.
