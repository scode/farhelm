# Extreme permitted history numbering can hang selection; desktop exclusion proposed with browser scope caveat

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Extreme peer-supplied history numbering can hang the page when the user selects text in a later screen row.

## Details

### Availability acceptance and browser scope

Additional reviewed commit: c5ae8548439cc5bfa33c11ef7ce8da6129d8072a. The acceptance referenced below is SPEC.md,
"Remote input, session defaults, and availability", 4045–4099, especially 4085–4092: denial of service of the helm or
desktop app by a malicious or arbitrarily broken host is accepted outright, even when Farhelm could avoid it. It retains
existing mitigations and does not accept corruption of the helm's state or effects on other hosts' security or data.
SPEC_impl.md, "Supervisor metadata retention and nonresponse", 76–90 explicitly points to that broader rule.

The rule expressly names the helm and desktop app; its application to this browser-only consequence remains ambiguous.
The separate bundled-terminal-library exception does not by itself settle Farhelm's own nonterminating selection loop.
This supplies the missing acceptance text and browser qualification referenced below. It is not a triage decision or a
bucket change. Provenance: review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g015/g015-l1`.

### Original mechanism and investigation

Suggested bucket: **high**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source mechanism
and loop consequence, not executed.

Source locations: crates/farhelm-ui/assets/grid-model.js:566-579,689-692 at 7509285e647ea86ccd2cee7905675dc87dad9b6f.

history first=Number.MAX_SAFE_INTEGER with rows=[] is allowed, sets historyExpected to that value, and the new
screenBase getter returns it. The next screen row has absolute number 2^53. JavaScript addition by one no longer
advances that number, so a nonempty selection within that row makes grid-buffer.js:123-145 loop forever at abs++ (even
missing lines take continue, which does not escape). tip grid-terminal.js:1044-1062 constructs pointer positions through
absAt (501-505), 1575-1586 extends a drag selection, and 1590-1609 calls selectionText on release; 1014-1018 passes the
range to this helper. grid-buffer.js:487-498 uses the same base. These grid-buffer.js/grid-terminal.js helpers are
unchanged between reviewed commit and tip, as checked with git diff --stat. The initial added test at
7d52f1cda4c301e8811acf23263b6ab66ce1930a grid-model.test.js:509-517 explicitly accepts the maximum empty first and an
append ending at the maximum.

at the reviewed commit grid-model.js:566-579 permits the maximum first or last history index and 689-692 exposes the
resulting screen base. At T equivalent code is crates/farhelm-ui/assets/grid-model.js:577-594,700-703. Later screen
absolute indices can reach 2^53, where addition of one stops advancing. crates/farhelm-ui/assets/grid-buffer.js:123-145
loops by abs++ through a selection, and 487-498 constructs its source using that base. The shipped pointer and release
path was established in the initial pass: T grid-terminal.js:501-505,1014-1018,1044-1062,1575-1609. A range within such
a screen row can therefore make the loop nonterminating; this is a source-established consequence, not a missing runtime
premise. No execution, disclosure, state corruption or user-work-loss route was found. Ordinary-producer synthesis: T
grid_output.rs:595-702 derives its numbering from the primary grid's actual lines_scrolled_into_history and held lines,
not user-selected numeric text; the initial cursor resets its numbering on layout changes. No practical ordinary route
to this extreme count is established.

Possible coverage and caveats: this exact hostile-host acceptance, with the browser-scope caveat; no matching Planned
item, unlanded plan, BUGS entry, queue item or triage decision found by the initial focused coverage searches.

Additional reviewed locations: g041/f1: crates/farhelm-proto/grid-fixtures/output/valid/history-last-line.json:3-5;
output/valid/history-empty-at-last-line.json:1; g043/f1: crates/farhelm-proto/src/grid.rs:1198-1217 at
5c39b2cd67629ecf7c80b0bc9a053144d1aac011; g156/f1:
7d52f1cda4c301e8811acf23263b6ab66ce1930a:crates/farhelm-ui/assets/grid-model.js:526-546.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g157/f2`, `g041/f1`,
`g043/f1`, `g156/f1`. Original append-only events, revisions, withdrawals and independent judgments are retained in that
run's catalogs. Locations refer to the named reviewed revisions; the original revision context is
7509285e647ea86ccd2cee7905675dc87dad9b6f; confirmed persistent at ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This
was a source-only review; runtime occurrence and frequency were not measured.
