# Snapshot memory checks have an investigated headroom uncertainty

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The memory tests may hide an excessive checkpoint allocation below an earlier process memory peak. After source
investigation, the physical headroom needed for an actual false pass in these fixtures remains unestablished.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. unverified after investigation

Source locations: crates/farhelm-terminal-emulator/tests/memory.rs:121-128.

memory.rs:121-128 subtracts starting lifetime VmHWM H from ending VmHWM, rather than accounting for starting RSS R; if
the snapshot peaks at P, the observed increment is max(0,P-H), omitting up to H-R from the transient. Assertions are at
:210-212 and :372-373. The restore test separately bounds this headroom at :414-421. Investigation and results:
inspected both actual assertion-bearing generators (:196-209, :353-362), the 128 KiB batching buffer and its drop
(:97-110), and the underlying feed/effects and grid allocation paths at the pinned tip. The generators emit small rows
and neither a large OSC nor title stream; grid storage grows in 1000-row increments and retains the rows
(vendor/alacritty_terminal/src/grid/storage.rs:65-138). Emulator feed routes and drains effects after each batch
(src/emulator.rs:151-160; src/effects.rs:151-163,197-213). These checks establish no multi-MiB temporary allocation in
the current generators that would hide the targeted 40 MiB-versus-36 MiB regression. g071/f4 concerns production sizing
with 10000-column linked rows, not these 200-column ordinary/fixup fixtures, so it supplies no missing pre-snapshot peak
producer here and is not a duplicate. Exact unavailable premise: whether H-R is large enough under the intended
runner/allocator to conceal an otherwise failing snapshot-memory increment; source alone cannot infer the process's
physical resident-page history. Evidence settling it would be retained pre-snapshot HWM/RSS observations from an
authorized run, or a concrete static allocation path creating the required headroom. Runtime reproduction is prohibited
in this charter. The prior confirmed designation applied only to the arithmetic omission; this revision explicitly
retracts any implication of a verified effective false pass.

The snapshot-memory tests may understate the memory needed to save a terminal if earlier allocation left significant
space below the process's historical peak. No actual false pass, production regression or user-work loss has been
established.

Possible coverage and caveats: no exact coverage found. At the pinned tip, SPEC_impl.md:2549-2597 specifies complete
snapshots and about 32 MiB snapshot transient; TODO.md:23-24 Planned is empty. Repeated targeted searches of SPEC.md,
SPEC_impl.md, TODO.md, plans/queue, BUGS.md, queue INDEX and files, and TRIAGE_OUTCOMES.md found no matching accepted
measurement limitation. No FILTER.md exclusion matches this test-evidence concern.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g083/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a;
persistence checked at ca9b0a19856dc136f63ad2d14f1076027edce1eb, with no intervening change to memory.rs.. This was a
source-only review; runtime occurrence and frequency were not measured.
