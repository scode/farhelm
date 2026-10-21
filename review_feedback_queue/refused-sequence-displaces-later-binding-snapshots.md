# A refused sequence can displace later binding snapshots

Reviewed commit: fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1

## TLDR

A refused sequence can leave its saved Enter bindings queued, so later sequences may use an earlier template name
despite displaying the new one. The physical stale-DOM timing needed to start this displacement remains unverified.

## Details

`crates/farhelm-ui/assets/template-shortcuts.js:160–175,231–251` arms from the rendered Begin button, then saves a
physical Enter snapshot. Rust independently rejects Begin from live pending/template/busy state
(`src/list/view.rs:3745–3750,3775–3799`). It ignores digits without a sequence and dispatches Enter only for a nonempty
queue. That leaves no worker request to consume the saved snapshot. The next keyboard batch shifts the orphan
(`list/view.rs:3470–3547`); rejecting its old context/epoch still leaves that batch's own snapshot queued. A later batch
under the same context can then consume the preceding projection, retaining its earlier name after bindings move.

One conditional trigger is a direct shortcut setting pending (`view.rs:3489`) followed by prefix/digits/Enter before
disabled/context publication reaches the DOM. After its opening closes, one sequence can reject the old epoch and the
following sequence consume the earlier ordinary projection. Modal cancellation does not drain the FIFO
(`modal_isolation.rs:152–161`). Correlate or retire captured presses even when no Rust sequence dispatches. Ordinary
blocked direct batches consume before rejection, so they do not share this exact omission.

Suggested bucket: provisionally **highest**, for possible wrong-host/folder command execution affecting user-owned work.
Confidence: **likely conditional**; missing consumption and persistent pairing displacement are source-supported, while
physical input/publication reachability and a harmful launch were not reproduced for browser or desktop. Preserving a
captured name prevents later GET rebinding; it does not establish correct pairing with this Enter.

The same reviewer subsequently classified native stale-Begin reachability as an unresolved source premise, not merely
missing runtime measurement. The pinned repository delegates event/render transport to Dioxus 0.7.10 and does not vendor
that bridge. No delivery ordering or invariant resolving the premise was established within its assigned scope. Its
discovery remains incomplete; this conditional record does not justify advancing the trailing-review checkpoint.

Possible cover: `SPEC.md:803–804` requires the displayed names; `SPEC_impl.md:4980–5005` pairs snapshots and drops busy
presses. Ledger fresh-binding and pre-repaint decisions at `TRIAGE_OUTCOMES.md:15953–15972,16402–16418` do not clearly
accept persistent displacement on later presses. General subsecond allowance may cover the initial stale action but not
the later pairing error; timing filters exclude wrong-target/work-loss effects. No exact Planned, BUGS or queue match
was found. Proposed drop: none.

Provenance: `audit-sequence-target-disclosure/audit-snapshot-1`, discovery pass 1. Its prior `audit-binding-1` exclusion
is amended to preserve this missing pairing premise and original `quick-launch-implementation-1/f1` provenance. No
runtime reproduction was performed.

Original finding IDs: `audit-sequence-target-disclosure/audit-snapshot-1`.
