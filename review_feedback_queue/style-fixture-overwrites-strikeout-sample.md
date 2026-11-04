# Styles golden fixture overwrites its strikeout sample

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

Current source status at c5ae8548439cc5bfa33c11ef7ce8da6129d8072a: a new independent frozen fixture added by 978b0183
covers strikeout. The original styles fixture is unchanged; the broader missing-coverage consequence below is
historical. This review records the repair for explicit queue cleanup.

## TLDR

The compatibility fixtures do not retain struck-through text, leaving that style outside their frozen-byte guard.

## Details

### Later coverage repair

Commit 978b0183 adds `style-removals-and-reset-tie` without regenerating old frozen bytes. At the additional reviewed
tip, `crates/farhelm-terminal-emulator/tests/snapshot_golden.rs:162–175` prints retained cells with SGR 9 and removes
strikeout separately with SGR 29. Its new `.state.txt` fixture at 16–23 explicitly retains STRIKEOUT, and its companion
hex blob fixes the encoded representation. The directory-based restore check and script-to-frozen-bytes check consume
these artifacts independently of current encoder/decoder agreement.

Confidence: **source-confirmed coverage repair**, with no golden tests executed by this review. The original overwritten
sample remains real, but no longer establishes that strikeout lacks a frozen-byte guard anywhere. The feedback file and
index survived the repair. Keep that lifecycle evidence here for an explicit disposition; this addition neither removes
the item nor reports a new production restoration bug. Provenance: review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`,
`g021/g021-3`.

### Historical fixture gap

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
missing golden coverage; no production restoration bug or measured test failure claimed.

Source locations: crates/farhelm-terminal-emulator/tests/fixtures/snapshots/v1/styles.state.txt:16-40.

tests/snapshot_golden.rs:112-133 feeds bold (4 columns), dim (3), ital (4), inv (3), hid (3) and SGR 9 st into a
16-column first row at line 117. hid already wraps; st lands on the next row, then the underline sample at line 120
overwrites it. styles.state.txt:16-40 records attributes through HIDDEN and then the underlines, with no STRIKEOUT;
styles.hex:1-26 likewise contains no SGR 9. git grep of all version-1 fixture descriptions found no STRIKEOUT sample
elsewhere. scripts_exercise_their_feature (snapshot_golden.rs:518-527,638-646) explicitly guards against samples
overwriting their intended features but checks combining characters, tab and emoji only for styles.

tests/snapshot_golden.rs:112-133 feeds hid then SGR 9 st into a 16-column first row (line 117); the hid text already
wraps, placing st on row 2, which the underline sample at line 120 overwrites. styles.state.txt:16-40 consequently
records the first-row attributes through HIDDEN and the next row's underlines, with no STRIKEOUT, and styles.hex
likewise contains no SGR 9. This is a test fixture premise gap, not a demonstrated production restoration bug.

The compatibility fixtures do not retain struck-through text, leaving that style outside their frozen-byte guard. Exact
source basis at both commits: tests/snapshot_golden.rs:112-133 uses a 16-column first row; the sample at :117 prints
bold (4), dim (3), ital (4), inv (3) and hid (3), then strikeout st. The third hidden character already wraps, so st
occupies the second row and is overwritten by the underline sample at :120. The snapshot is taken after all script
steps. tests/fixtures/snapshots/v1/styles.state.txt:16-40 consequently records no STRIKEOUT, and styles.hex:1-26
contains no SGR 9. A git grep across every version-1 fixture at tip found no STRIKEOUT sample. The feature-premise
assertions at snapshot_golden.rs:638-646 check combining characters, tab and emoji, leaving this overwritten sample
unnoticed.

Possible coverage and caveats: none found. Repeated immutable-tip searches cover SPEC.md, SPEC_impl.md, TODO.md,
BUGS.md, plans/queue, queue INDEX and all queue files, and TRIAGE_OUTCOMES.md with
strikeout/strikethrough/struck-through/snapshot_golden/styles.state/styles.hex/golden-fixture terms. Full relevant
sections already read: SPEC_impl.md:2549-2597 requires permanent byte-for-byte goldens and does not accept this gap;
TODO.md Planned:23-24 is empty; its separate checkpoint-performance entry :104-132 constrains emitted bytes without
repairing the sample. Other golden-fixture search hits concern protocol fixtures or general permanent-surface rules, not
this trigger/consequence/scope. FILTER.md has no applicable test-fixture-coverage exclusion. Synthesis: the full fixture
directory and consumer preserve the same omission at tip; current painter STRIKEOUT support remains correct (tip
src/snapshot/v1/paint.rs:687-700, assigned commit :654-667). Property round trips generate SGR parameters
(tests/common/output.rs:125-126; snapshot_roundtrip.rs:114-133), which can exercise current encoder/decoder agreement
but do not supply a frozen strikeout blob. g087's findings log was empty when inspected, so no duplicate consumer
finding was present. g173/f1 concerns downstream grid serialization discarding styled leading-wide placeholders, a
distinct attribute and mechanism; do not merge it into this fixture-premise issue. Suggested correction: add a new
strikeout fixture with an explicit retained-cell premise; do not regenerate existing frozen fixtures.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g080/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a;
unchanged through inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (git log for the styles fixtures and
snapshot_golden.rs returned no subsequent commits).. This was a source-only review; runtime occurrence and frequency
were not measured.
