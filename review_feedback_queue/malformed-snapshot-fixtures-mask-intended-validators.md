# Malformed-value fixtures are rejected before their intended validators

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Several terminal-snapshot corruption fixtures never exercise the validation they name, so they remain green after those
checks regress.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by static
control flow, not a hypothetical schedule or measured flake.

Source locations: crates/farhelm-terminal-emulator/tests/snapshot.rs:447-504 at
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a.

the unknown named color case ends at u16(999), and unknown color kind ends at u8(3), before completing the cell, cursor
count and REP sections. A decoder that accepts the intended bad value can still return Malformed for later EOF.

Several terminal-snapshot corruption fixtures never exercise the validation they name, so they remain green after those
checks regress. Original snapshot.rs:447-456,458-467,469-471,473-486,504 contain only 16,14,13,19 cell-record bytes or 2
cursor-record bytes. Original snapshot/v1/mod.rs:140-146,416,431 checks 22/28 minimum sizes before decoding, through
wire.rs:549-557; snapshot.rs:618-625 accepts any Malformed. At tip the same fixtures are
snapshot.rs:450-459,462-470,473-474,477-489,507; size gates are snapshot/v1/mod.rs:141-147,434,449 and wire.rs:549-557.

Possible coverage and caveats: partial only. wire.rs:742-757 independently tests unknown flags. Also snapshot.rs:505-521
at the original commit (508-524 at tip) provides a complete invalid-wrap-boolean case that exercises the shared
Reader::bool helper, so do not claim that helper has no regression coverage. The incomplete saved-cursor boolean case
still does not test its own call site; named-color, color-kind and invalid-scalar cases remain concrete ineffective
fixtures. Remedy: complete otherwise-valid records and suffixes, mutate only the intended field, and add valid
companions or exact refusal assertions. Coverage sources searched previously at pinned tip: SPEC.md, SPEC_impl.md,
TODO.md Planned (empty :23-25), plans/queue, BUGS.md, queue INDEX/files, TRIAGE_OUTCOMES.md. No exact coverage/filter
for the remaining fixture defect. SPEC_impl.md:2549-2598 requires typed refusal and valid fixups;
TRIAGE_OUTCOMES.md:9836-9852 concerns a completed different helm EOF-oracle defect.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g084/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a;
persistence verified directly at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime
occurrence and frequency were not measured.
