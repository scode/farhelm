# A queued launch may capture a binding before its indicator repaints

Reviewed commit: fe576b54cc233f16ce2bdd454057570c5d6ceec1

## TLDR

If changed shortcut settings can reach the Enter handler before the indicator repaints, a queued launch could briefly
capture a newly bound template while the old name remains visible. This scheduling premise and any harmful launch are
unverified. The original indefinitely stale shortcut-resolution defect is fixed.

## Details

`crates/farhelm-ui/src/list/view.rs:3743–3752` captures queued template references from the current
`preferences.0.peek()` when Enter arrives. The indicator independently reads that signal during render
(`crates/farhelm-ui/src/launch_sequence.rs:351–392`). A signal change followed by Enter before the corresponding DOM
update could therefore capture B while the indicator still shows A. If the templates differ in host or folder, this
could dispatch a launch somewhere the visible sequence did not name. No actual event/render interleaving or wrong-target
launch was established.

The implemented fix synchronously converts keys to `Name` or `Unbound` before the asynchronous fresh reads
(`launch_sequence.rs:262–280`; `list/view.rs:3516–3536`). It closes the original long-lived rebinding gap. Current
template definitions are intentionally read afresh; that is not the residual in this record.

Possible cover is substantial. `SPEC.md`, “One GUI at a time,” `3163–3166`, expressly accepts subsecond actions before
redraw while preserving explicitly named guards. The ledger's `queued-launch-can-change-template.md:15918–15936` records
the user's original “IS covered” decision and choice of a cheap fix with a complexity gate. Its completion criteria and
`SPEC_impl.md:4855–4865,4880–4884`, however, expressly describe capturing the indicator's shown snapshot and displayed
bindings. A fresh independent auditor could not determine whether that wording overrides the generic redraw acceptance.
Resolve that contract before commissioning stronger capture machinery. A deterministic event/render check would also be
needed to establish the premise.

Suggested bucket: provisionally `highest`, solely for the conditional wrong-host/folder consequence. Confidence:
plausible source-level boundary with unverified scheduling and harm; this is not a reproduced regression. No other exact
Planned, BUGS or queue coverage was found. The ordinary subsecond-race filter excludes wrong-target and trust-boundary
consequences, so it does not settle the conditional case.

Proposed drop: the primary reviewer proposed the generic redraw acceptance; an independent fresh-context auditor
returned ambiguous and required retaining the narrow residual with the acceptance caveats. Provenance: separate
same-model UI reviewer and independent same-model exclusion auditor in the trailing review through
`bf0935f02fc8fbfc26ab54e1b9fb8e7e6bf5c441`; the recording parent checked the two readers and complete cited acceptance
sections. No runtime reproduction was run.
