# Short-window outage explanations block pointer recovery

Reviewed commit: 9309f15e1aff92d07be0531326f9bfc93f067ad2

## TLDR

When an unavailable session has less than 80px of terminal space, its explanation covers the sidebar controls needed to
recover that host or select a healthy session. Keyboard navigation and enlarging the window remain workarounds.

## Details

`crates/farhelm-ui/assets/outage-layout.js:7` selects the fallback below 80px terminal height.
`app.css:7032–7043,7061–7065` makes the pointer-catching overlay fixed across the viewport at layer 6; the sidebar app
bar is layer 5 (`714–723`). The inspected ancestors establish neither a containing block that confines this fixed
surface nor an isolating stacking context. The normal closed-menu state leaves independent sidebar controls beneath its
hit area. An already-open higher-layer menu may be an exception.

`crates/farhelm-ui/src/session_view.rs:3331–3347` gives the unavailable-host card no dismiss or host-recovery action.
Identity mismatch, duplicate entry and retired-actor causes direct the user to Adopt, destination editing, Remove or
Retry (`hosts.rs:844–867`). Those controls are outside the card. Preserve a pointer route to recovery and other sessions
while keeping the explanation readable.

The added test at `e2e/tests/terminal-multihost.spec.ts:3168–3199` verifies fixed placement, heading hit testing and
keyboard/wheel text endpoints. It does not click independent host controls or a healthy session in that state. The
reconnect test restores a tall viewport before its pointer click (`terminal-reconnect.spec.ts:613–625`). These are valid
explanation-access oracles, not navigation oracles.

Suggested bucket: **high**, for material pointer navigation/recovery obstruction. Confidence: **confirmed static
CSS/markup mechanism; browser behavior unexecuted**. This is not an all-input permanent trap, security claim or loss of
user work.

Possible cover: `SPEC_impl.md:547–553` at the final review tip explicitly gives the card the viewport, and
`TRIAGE_OUTCOMES.md:16767–16785` records placement above the sidebar. These genuinely accept placement. They do not
expressly accept blocking independent controls; the recorded decision says controls and explanations remain reachable.
`SPEC.md:1504–1537,2688–2691` retains host actions and remedies. Keep this interpretation question for triage. No exact
Planned, BUGS or current queue match was found. FILTER's scroll-reachable-explanation exception does not cover unrelated
controls blocked by the hit area, and the condition can persist. Proposed drop: none.

Provenance: trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`; independent dialog implementation,
contract, report and test reviewers corroborated the same mechanism during later-turn synthesis. Their reports are one
underlying finding, not separate bugs. No runtime reproduction was performed.

Original finding IDs: `dialog-layout-contracts-1/f3`, `dialog-layout-contracts-2/f1`,
`dialog-layout-implementation-1/f1`, `dialog-layout-tests-1/f1`.
