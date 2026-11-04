# Discord header hides the Send pointer target in short feedback windows

Reviewed commit: addba1f54182a1462da2af0791bcc51c29b12e2e

## TLDR

The added Discord header can hide the feedback dialog's Send button from pointer interaction in a short, narrow window.
Keyboard Tab can reveal the button and sending remains possible; the confirmed regression is losing a previously visible
pointer target.

## Details

The feature is compared with first parent `831190acac09e8c1131cda4b2bc2fad356ede47e`.
`crates/farhelm-ui/src/feedback.rs:251` renders the framed feedback dialog without `host-settings-dialog`. Its extra
summary at `272–290` and `crates/farhelm-ui/assets/app.css:3320` increase a nonshrinking header. The framed root clips
overflow at `app.css:3951–3955`; the whole-card scrolling and nonzero-body fallback at `4120–4133` only protects the
host-settings class. The added header therefore lowers the window height at which the editing controls become clipped.

Inspected source-matched static fixtures cover all 12 baseline/feature viewport measurements in each engine:

- WebKit run `26548a13-c852-4da4-bb85-658738167ffc`, complete 483,914-byte output, exit 0.
- Chromium run `18deb4ec-4b4f-41e9-b3b2-01e4307d1f5e`, complete 482,266-byte output, exit 0.

At 360×280 the baseline's Send target has 19.203125px visible height and a hittable center, while the feature has zero
visible height and an unhittable center. At 380×280 the baseline's 32.796875px button is fully visible, while only
0.203125px remains visible in the feature and its center is not hittable. The feature's header grows 113→147px at width
360 and 98→132px at width 380. Both engines show the same results. Body-wheel scrolling leaves the clipped footer
unchanged.

Native Tab reaches contact, reuse, Cancel and Send in every measured pair. Focusing Send scrolls the feature root 62px
at 360×280 and 33px at 380×280 and reveals it fully; raw pointer activation succeeds after that keyboard recovery. At
taller measurements the feature retains progressively more of Send and it is fully visible at 380×320. The baseline
already clips some content at smaller heights; the established regression is the demonstrated loss of a previously
visible pointer target.

`SPEC.md`, “Feedback/Sending feedback,” 3326–3397 requires an operable message/send dialog. `SPEC_impl.md`, “GUI:
Dioxus,” 554–570 preserves scrolling and action ownership. Extending the existing whole-card scroll/body-floor fallback
to feedback is a proposed fix direction. Verify combined short/narrow editing hit targets and natural keyboard recovery,
rather than only a sent-state structural-scroll check.

## Confidence and priority

Confidence: confirmed source-CSS regression in source-matched static Chromium and WebKit fixtures. The fixtures use
immutable production CSS/fonts and copied editing DOM; they omit compiled Dioxus modal IPC/isolation and real
submission. A successful probe means measurement premises and collection passed, not that controls stayed visible. No
total inability to send or measured compiled-app failure is claimed.

Suggested bucket: `other`, for recoverable small-window pointer visibility. `could_be_highest: false`.
`proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established; its small-window explanation
filter does not cover a hidden Send action.

Grouped findings: `g028/g028-l1` and `g053/g053-l5` identify this same hidden pointer-target regression.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, owner `g028`, ID `g028-l1`, latest `g028/findings.jsonl` physical
line23; grouped source finding `g053/g053-l5`. Related assigned plan/reports:
`5e155da11b72c25bde82c5b34daff9887cf233ee`, `86bd0f2ca010d1aa4d492b267017b73ff8162a81`,
`9d3f9874cc32c7cd9b9f174921564aa0a8005f58`. Earlier selector-based refutation and unmeasured-geometry uncertainty were
superseded by the inspected bounded measurements.
