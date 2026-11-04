# Replay work accounting omits growing history rotations

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The checkpoint reader undercounts repeated history-buffer rearrangement for unusually large library-requested
scrollback. No excessive latency at shipped settings or shipped producer for the extreme history count is established.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed omitted source
cost; supported-scale latency unmeasured

Source locations: crates/farhelm-terminal-emulator/src/snapshot/v1/replay.rs:611-618.

the snapshot reader's claimed work estimate misses repeated moves of the growing history buffer. The library permits a
caller to request much larger scrollback than the supervisor default, and a narrow terminal with that much retained
history can take substantially more restore work than the estimate accounts for. No excessive latency, supervisor
availability problem, security impact or user-work loss is established for shipped settings. Details/source basis at
c345799: replay.rs:611-618 charges a scroll 2*columns plus screen height for a partial region, calling thousand-row
history growth amortized linear work. Token::LineFeed at :629 invokes that charge, while :73-76 makes the permitted
budget linear in bytes and grid cells. vendor/alacritty_terminal/src/grid/mod.rs:228-232,324-347 increases the history
limit before rotating; grid/storage.rs:67-75 initially allocates only screen rows, and :126-137 grows its initialized
rows in 1000-row steps, first calling rezero. At :236-244 rezero calls inner.rotate_left(self.zero), rearranging the
existing Vec<Row> rather than only updating the ring index (the storage docs explicitly distinguish these costs at
:15-19). Across successive thousand-row growth steps, the rearranged arrays get progressively larger; their cumulative
row-move cost grows quadratically in retained history rows divided by 1000. The model has no charge for this growth. The
ordinary steady-state ring rotation at storage.rs:180-185 is constant-time and is not the problem. The decoder at
c345799 snapshot/v1/mod.rs:375-381 limits retained history by the caller's scrollback and restorable cells, not a
12000-row ceiling. snapshot/mod.rs:441-450 allows large narrow-terminal histories under its cell limit, and underlying
Grid::new at grid/mod.rs:158-168 stores the supplied history limit without another cap. At tip
ca9b0a19856dc136f63ad2d14f1076027edce1eb, the public restore contract at snapshot/mod.rs:553-566 still accepts
caller-provided usize scrollback; :593-607 caps by allocation cells/1000-row steps rather than the supervisor default.

Possible coverage and caveats: no exact coverage found after searching SPEC.md, SPEC_impl.md, TODO Planned, unlanded
plans/queue, BUGS.md, queue INDEX/matching source text and TRIAGE_OUTCOMES.md, and reading queue INDEX:1-1170 in full.
TODO Planned is empty (:23-24); Near-term TODO:104-132 addresses encoder latency, not restore-history growth.
SPEC_impl.md at tip, Terminal snapshots :2549-2597, expressly requires a stream/grid-derived work limit (:2571-2576) and
does not cap the library's restore contract at 12000. SPEC.md:3991-3994 accepts supervisor availability effects from
hostile same-account processes, but this finding is the source-level library bound claim and makes no
availability/security allegation. FILTER.md's transient diagnostic/display/failure filters do not fully describe the
omitted work accounting.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g073/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a.
Persistence: replay.rs, vendor/alacritty_terminal/src/grid/mod.rs and grid/storage.rs are unchanged through
ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and frequency were not
measured.
