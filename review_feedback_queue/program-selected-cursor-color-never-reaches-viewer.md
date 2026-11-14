# Program-selected cursor colors never reach grid viewers

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A program that changes its text cursor color continues to show the theme's fixed cursor color, even though Farhelm's
emulator accepts the new color.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static source
behavior, no runtime frequency/experiment premise required.

Source locations: crates/farhelm-supervisor/src/grid_output.rs:348-355,462-477.

A program that changes its text cursor color continues to show the theme's fixed cursor color, even though Farhelm's
emulator accepts the new color. Details/source basis at tip: vendor/vte/src/ansi.rs:1471-1498 maps OSC12 to
NamedColor::Cursor and calls set_color; queries.rs:283-295,342-343 delegates it unchanged;
vendor/alacritty_terminal/src/term/mod.rs:1855-1863 stores it. Assigned grid_output.rs:348-355 serializes
position/visibility/shape/blink, while :462-477 serializes only default foreground/background colors. Protocol
grid.rs:969-1006,1052-1083 has no cursor-color field. Browser grid-style.js:220-239 gets cursor color solely from
theme.cursor or defaults.fg; grid-terminal.js:763-775 (read in synthesis) paints with that mapper color. No transmitted
value can represent the accepted OSC12 change, on live update or reconnect. R's parser and mapper excerpts independently
confirm the same route before tip.

Possible coverage and caveats: none found. Required coverage searches at pinned tip across SPEC.md, SPEC_impl.md, TODO
Planned, all unlanded plans/queue, BUGS.md, queue INDEX and TRIAGE_OUTCOMES.md found no matching accepted exception,
cause-removing plan or duplicate. Retained full SPEC.md Terminal experience2080-2391 promises color fidelity
at2085-2086. Full SPEC_impl grid-client1706-1736/stream2442-2482/emulator2517-2548 sections do not accept ignoring
cursor colors; queries.rs:1-146 explicitly lists OSC12 queries as the program's own color if set, with no
unsupported-setting caveat. The protocol's closed Cursor/TerminalState schemas document the missing channel, not
explicit acceptance of its user-visible loss. TODO Planned23-25 remains empty. TRIAGE_OUTCOMES11316-11326 concerns
save/restore formatting wholly inside vendored xterm.js, not this Rust-to-grid omission; read in full and not treated as
coverage. FILTER rare-self-correcting rule does not cover an ordinary program color change that stays wrong after
refresh/reconnect. Cross-run dedup search of current findings.jsonl files for cursor-color/OSC12 matched only this f4.
Distinct from g173/f1 (dropped styled leading cell) and g158/f1 (raw/effective bright palette selection), and no
existing local ID is reused to claim novelty.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g173/f4`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41
(assigned encoder introduction). Persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb: empty diff for
grid_output.rs, grid-style.js and vendor/vte ansi parser; emulator query/term diffs only add unrelated documentation..
This was a source-only review; runtime occurrence and frequency were not measured.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g191/l1`.

### g191/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e; assigned deletion is
crates/farhelm-ui/assets/vendor/xterm.js:2, module 9078 (ThemeService). Discovery pass 1. TLDR: a terminal program can
change its cursor color, but Farhelm continues drawing the bundled cursor color. A program querying the cursor color can
receive its requested color even though the viewer draws another. This is a replacement-contract gap exposed by
reviewing removal of the service that maintained cursor colors. Details: vendor/vte/src/ansi.rs:1486-1511 explicitly
handles OSC 10/11/12 through NamedColor::Cursor, dispatching set_color or a query. The emulator forwards
set_color/reset_color (farhelm-terminal-emulator/src/queries.rs:357-358);
vendor/alacritty_terminal/src/term/mod.rs:2145-2154 confirms set_color stores the cursor override in self.colors even
though cursor-color changes do not mark all cells damaged. queries.rs:476-486 answers dynamic queries from those stored
colors before theme fallback. However, supervisor grid_output.rs:329-362 builds Cursor from position, visibility, shape
and blink only; :461-477 sends foreground/background overrides but no cursor-color override. Proto
grid.rs:1040-1048,1128-1153 likewise has no cursor-color field. On the page grid-style.js:225-233 always returns
theme.cursor, and grid-terminal.js:792-819 uses that color for blocks, bars, underlines and unfocused outlines. Thus no
cursor-color update reaches the painter. The removed ThemeService contains mutable cursor state and restore handling;
parent xterm.js InputHandler registers OSC 12, setOrReportCursorColor raises the color event, and browser core routes it
through ThemeService.modifyColors, confirmed by direct minified-source reads. Contract: SPEC.md:2860-2861 says 'Full
fidelity: colors, cursor movement, alternate screens, resize. If it works over plain SSH it must work here.'
SPEC_impl.md:2689-2693 says color queries answer from the theme or colors the program set; the set value can
consequently disagree with the viewer. The complete Grid streams section (:3009-3110) contains no cursor-color
exception; grid_output.rs:42-48 names accepted blink differences, not this omission. Fix direction: carry effective
cursor color with reset semantics in the grid state/frame and have paintCursor use it. Suggested bucket: other,
cursor-color/display correctness. Confidence: confirmed by source and contract analysis; no runtime behavior observed or
reproduced. proposed_drop: false. possible_cover: none in supplied neighbour records; prohibited coverage sources were
not searched. Could-be-highest: false for the established cursor-color display/query discrepancy; no security or
loss-of-work route established. Attribution caveat retained: this omission already exists at range baseline
6e423e30068b62c6cfa276c6d720ee35bb70387a in grid-style.js:220-250 and grid_output.rs:460-476. Complete baseline-to-tip
diffs for both files change comments only; git log lists only f560058d and 61e63232 touching them. This is NOT a new
cursor-color regression for existing grid terminals introduced by f560058d. It is reported only as a checked
replacement-contract gap of the removed ThemeService, under the charter's deletion/replacement review rule; the caller
must retain that scope and attribution. No independent retired-code defect was sought. Earlier c3 establishes
parser/theme ownership, not preservation of every removed ThemeService capability. No parent drop or lowering is claimed
accepted.

Returned proposal (claims-120, event 14): **covered**, suggested bucket **other**.

The omission is source-established, but the exact trigger, consequence and scope are already recorded in the queue. The
existing item already says it affects both live updates and reconnects.

Original stronger consequence: Cursor-color display/query disagreement, other throughout. Earlier parser/theme ownership
refutation never established preservation of every ThemeService capability.

Unresolved premise or limitation: None for the conditional source omission. No runtime occurrence/frequency was
measured. The current review does not establish a newly introduced omission within this range.

Proposed basis: Exact existing-queue coverage, not source refutation or an accepted product exception. Optional
provenance extension may add f560058d's retained replacement-gap confirmation and baseline attribution; it adds no new
user-visible consequence.

Existing parent disposition (event 14): rejected. Parent explicitly accepted named covered proposal from claims-120
after inspecting it. The omission is source-established, but the exact trigger, consequence and scope are already
recorded in the queue. The existing item already says it affects both live updates and reconnects. Exact existing-queue
coverage, not source refutation or an accepted product exception. Optional provenance extension may add f560058d's
retained replacement-gap confirmation and baseline attribution; it adds no new user-visible consequence. Checked
source/results: ["At f560058da19e02716e6f943883a9508b447aff4e, vte ansi.rs:1486-1511 accepts OSC 12;
term/mod.rs:2145-2154 stores the cursor color; queries.rs:357-358 delegates it and :476-486 answers queries from stored
colors.","grid_output.rs:329-362 and :461-477 omit cursor color; proto grid.rs:1040-1048,1128-1153 has no field.
grid-style.js:225-233 returns theme.cursor and grid-terminal.js:792-819 paints it.","Baseline grid-style.js:220-240
already has the same fixed-theme mapping. Complete baseline-to-tip diffs for grid-style.js and grid_output.rs change
comments only. It is not a new f560058d regression for existing grid terminals; retain the supplied
deletion/replacement-contract attribution caveat. No runtime reproduction."] Current coverage/results: ["Current
SPEC.md:2860 requires fidelity; SPEC_impl.md:2670-2704 describes program-set color query answers and its complete Grid
streams section has no cursor-color exception.","TODO Planned empty; BUGS unrelated. Matching plans/queue sections
remove obsolete code/docs and retain cursor defaults but do not specify transmission of OSC 12
overrides.","INDEX.md:1315-1317 points to program-selected-cursor-color-never-reaches-viewer.md, read fully. Its
existing g173/f4 evidence covers the same accepted OSC12 -> omitted field -> theme-only painter chain, with
live/reconnect scope. Active TRIAGE_OUTCOMES search found no further exact decision. FILTER rare/self-correcting does
not cover an ordinary program color change remaining wrong after reconnect."] Exact cited cover:
["review_feedback_queue/program-selected-cursor-color-never-reaches-viewer.md, TLDR: A program that changes its text
cursor color continues to show the theme's fixed cursor color, even though Farhelm's emulator accepts the new
color.","Same file, Details: No transmitted value can represent the accepted OSC12 change, on live update or
reconnect."] Original stronger consequence and missing premise remain preserved: "Cursor-color display/query
disagreement, other throughout. Earlier parser/theme ownership refutation never established preservation of every
ThemeService capability."; "None for the conditional source omission. No runtime occurrence/frequency was measured. The
current review does not establish a newly introduced omission within this range.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g191 record](records/review-through-00de0b6c/groups/g191.json). Record ID: `l1`.
