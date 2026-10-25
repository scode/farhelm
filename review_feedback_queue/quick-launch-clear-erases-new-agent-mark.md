# Clearing a mark can erase a newer agent choice

Reviewed commit: 038e2f0eb6f561baaf5686a4af1af31f1b7b138f

## TLDR

An agent can move the quick-launch mark after the GUI's fresh read but before its unconditional clear, losing the newer
choice. Broad last-writer-wins wording and the narrower preservation rule conflict.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/templates.rs:1646-1653`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Discovery verification complete for reviewed commit 038e2f0eb6f561baaf5686a4af1af31f1b7b138f. TLDR: clearing template A
can erase a newer quick-launch mark assigned to B by an agent or the command line. Distinct from a047/f1: shown Boolean
can be captured correctly and never change before the handler; the race is between the handler GET and its later PUT.
templates.rs:1637 reads A; 1646-1653 turns clear-A into QuickLaunchTemplate(None); 1688 queues it. api.rs:1248-1250
sends only the nullable field, no expected owner. helm preferences.rs:249-250 applies it via update_preferences;
store.rs:5196-5197,5247,5278-5279 unconditionally writes NULL when the field is present. An agent mark-B write can
commit after the GUI GET and before the clear PUT: the GUI does not change durable A until that PUT, so the agent
expected-A precondition can still pass (templates.rs in helm:132-145 routes conditional binding writes). This leaves
durable mark None, not B. No product SOURCE premise remains missing; exact interleaving frequency is unmeasured. Bucket
other, confirmed conditional source mechanism. No automatic launch, wrong-machine/session action, security or user-work
loss established. possible_cover: pinned SPEC_impl.md:254-255 broadly says separate clients remain last-writer-wins, but
SPEC.md:3423-3432 explicitly forbids undoing concurrent agent/CLI changes and limits its exception to templates and
number keys. Ledger template-preservation-promise-omits-last-write-wins.md at TRIAGE_OUTCOMES.md:17602-17624 confirms
that narrow exception. Coverage is therefore AMBIGUOUS rather than clearly accepted; preserve for triage. Older
quick-launch HTTP-overtaking discard is a different trigger/consequence. Failed-settings-save plan covers failures, not
this successful null write. Rare display/diagnostic filter does not cover durable changed assignment; no human
sub-second action after the window opens is required. proposed_drop: none. Six required coverage sources searched at
pinned tip; Planned is empty, no matching BUGS or queue item found. This is a new post-read ownership race, not a second
polarity variant.

Provenance: `a047/f4`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
