# A late host Suspend question interrupts subsequent typing

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain high. `hosts.rs:2437–2461` still awaits
activity then opens Suspend without a moved-on baseline; `3553–3558` still uses HostModal with remembered Cancel focus.
`modal_isolation.rs:75–105` gates other modal roots, with no activity check. The new per-host launch exclusion changes
dispatch concurrency rather than this presentation permission. `SPEC.md:3240–3246` still forbids this moved-on opening.
Reservation/Add-refusal coverage concerns competing modals, not this sole-dialog typing interruption. No exact current
coverage or later repair was found; the earlier focus-plan discussion below is historical.

## TLDR

After choosing Suspend in Hosts, the user can resume typing while Farhelm checks activity. A late reply can then open a
modal question over that work and take keyboard focus instead of waiting to be reached.

## Details

Bucket: **high**. Confidence: **confirmed source route**, with no runtime timing measurement. The reviewer suggested
high for active-work interruption. Cancel is the default; no accidental suspension, unsafe answer, security consequence
or user-work loss is claimed.

Reviewed source change: `fa518784fc3773bfdc5fc5cdf01269d3858f6b78`, the assigned Suspend slot migration; the final focus
target is introduced at `6e423e30068b62c6cfa276c6d720ee35bb70387a`. This does not assert that migration first introduced
the older delayed-opening policy.

At the reviewed tip, `crates/farhelm-ui/src/hosts.rs:4878` dispatches Suspend through `on_lifecycle`. Lines 2270-2297
close the menu and await `fetch_sessions` in a spawned task with no modal yet. Working, stale, unknown or failed
activity reaches 2315-2316 and opens the question without capturing or checking intervening input. Render 3417-3424
supplies `HostModal` with default `require_active=false` and remembered Cancel focus.

`modal_isolation.rs:80-119` waits for another modal to leave. Its optional active-window condition is unused here and
would not measure moved-on input anyway. `modal_dialog.rs:999-1004` installs mount focus; remembered-focus handling at
780-808 selects Cancel before isolation at 830-837. Correct opening-token answer binding does not veto presentation.

`SPEC.md:319-337` requires a question for uncertain or working activity, but not interruption of subsequent work.
“Keyboard focus”, 3212-3219 requires a later result after intervening input to open no dialog and wait in place or
behind a notice. Bind this prompt's presentation to initiating activity while preserving its required confirmation and
allowing the user to reach it later.

The earlier yolo-focus-origin proposal explicitly excluded the Hosts-panel Suspend question. At recording-time main
`cb03def00870e18a98a1fd9046073d2849be3253` that plan is abandoned, and the broad asynchronous-focus TODO has been
removed in anticipation of a GUI rewrite; neither supplies a repair or remaining planned coverage. The
host-dialog-reservation and Add-host-refusal queue items and current ledger entries at `TRIAGE_OUTCOMES.md:995-1034`
concern competing dialogs that become mutually inert; this route needs no other dialog. Their older dialog-order
convenience principle does not clearly accept violating the final moved-on opening rule. Following their historical
citation, the archived delayed-host-dialog decision requires one modal at a time, not opening over later terminal input.
No exact accepted bug, other unlanded repair or filter cover was established; the activity read has no shown subsecond
human-action bound.

This presentation owner and repair differ from a returned failed launcher and from an inline question's body-focus
fallback. Keep them separate even though all use the final activity rule.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `late-host-suspend-modal`; `g001/g001-l5`, including
event20 and its lead history in `prep/groups/g001/findings.jsonl`. No runtime or live-install action ran.
