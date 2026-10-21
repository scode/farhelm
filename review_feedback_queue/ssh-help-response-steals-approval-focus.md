# SSH help responses take focus from approval answers

Reviewed commit: 077b639d00a8dc177dcd6948283d286237b24868

## TLDR

While SSH help loads, a user can focus an agent's approval answer. When the commands arrive, focus moves to help's Close
button. No unauthorized approval or loss of work was established.

## Details

`crates/farhelm-ui/src/ssh_help.rs:68–78` adds response-driven focus recovery whose usability predicate requires the
active element to be inside the help dialog. A usable approval answer outside it fails that predicate. The same revision
deliberately exempts approval input from modal isolation (`modal_isolation.rs:36–44,183–195`), marks the approval region
exempt and renders native answer controls (`approvals.rs:156–170,448–479`), and places it above the backdrop
(`assets/app.css:2816–2825,7511–7519`). Approval layout does not exclude an open help dialog.

The read has a ten-second deadline (`api.rs:1695,1759–1770,3882–3899`), so this is bounded but need not be a subsecond
race. A valid response can arrive after the user has held independent approval focus for seconds. Preserve usable focus
in modal-exempt controls before recovering lost dialog focus. A later regression can hold a command-containing help
response, focus an enabled approval answer, release the response before expiry, and inspect focus after the animation
frame. Existing held-response cases check the help body and dismiss control, not an exempt owner.

Suggested bucket: **other**, for focus interference. Confidence: **confirmed static mechanism and reachable UI contract;
runtime unexecuted**. A following Enter might dismiss help, but no wrong-target mutation, unauthorized answer, security
impact or user-owned-work loss was established. Proposed drop: none.

Possible cover: `SPEC_impl.md:583–594` at the final review tip preserves the exemption. `SPEC.md:2716–2725` and
`SPEC_impl.md:1612–1621` describe help without accepting this takeover. The ledger's
`ssh-help-close-loses-replaced-opener-focus.md` concerns closing help, not command arrival. The existing notification
handback record has a different trigger and consequence. No exact Planned, BUGS or queue match was found. The
near-simultaneous user-input allowance does not establish acceptance of an HTTP response overriding focus, and the
subsecond filter's input-window premise is not established by a ten-second deadline. The rare-glitch filter does not
enumerate focus takeover among its complete consequences.

Provenance: trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`; dialog implementation and test reviewers
independently reconciled the response callback, exempt controls and narrower fixtures during synthesis. No runtime
reproduction was performed.

Original finding IDs: `dialog-focus-contracts-1/f1`, `dialog-focus-contracts-2/f1`, `dialog-focus-implementation-1/f1`,
`dialog-focus-tests-1/f1`.
