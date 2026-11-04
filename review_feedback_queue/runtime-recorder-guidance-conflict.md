# Conflicting recorder guidance for ignored emulator runtime tests

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Run-all-tests gives conflicting directions about whether emulator stress and memory tests require the runtime-test
recorder; taking the optional direction can leave future failures without the normally required retained run record.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conflicting
committed guidance only; no actual historical evidence loss, improper executed bypass or product regression is alleged.

Source locations: c345799ed43ddbd6e0c577f8f2a0add4dd051c6a:.agents/run-all-tests.md:120-122.

the assigned insertion at .agents/run-all-tests.md:120-122 adds product-emulator runtime tests beneath the section-wide
optional-recorder sentence at :75-80; the tip preserves these at :127-129 and :80. Both the introducing commit and tip
retain the all-runtime-tests requirement in AGENTS.md:237-240 and .agents/test-evidence.md:11-25. The latter
additionally says inventory commands are child argv after the recorder's --. The specific-flow sentence can be
interpreted as an exception, as g005/C8 did, but it conflicts with the user-supplied root rule and its unqualified
source copy; no explicit runtime-test exemption or reconciliation was found. This is a confirmed committed-guidance
conflict, not proof of a past instruction violation, lost artifact, historical run, or product regression.

introducing .agents/run-all-tests.md:120-122 adds these runtime tests under the section-wide optional recorder sentence
at :75-80. AGENTS.md:237-240 and .agents/test-evidence.md:11-25 require recording runtime tests and identify inventory
commands as child argv after --. Tip retains the entry at .agents/run-all-tests.md:127-129, optional direction :80, and
contrary rules unchanged.

the assigned patch adds ignored emulator runtime tests to .agents/run-all-tests.md:120-122 beneath the section-wide
optional-recorder rule at :75-80. At pinned tip the same insertion is :127-129 and optional rule remains :80;
AGENTS.md:237-240 and .agents/test-evidence.md:11-25 require the recorder for local runtime tests. The tests are product
emulator runtime tests, not source-only checks.

Possible coverage and caveats: TRIAGE_OUTCOMES.md at tip:20479-20495 excludes operational artifacts/validation
accounting, but whether that covers errors in future procedural instructions remains ambiguous. Prior focused searches
of SPEC.md, SPEC_impl.md, empty TODO Planned, unlanded plans/queue, BUGS.md, queue INDEX/matching files and
TRIAGE_OUTCOMES.md found no exact cause-removing fix or unambiguous exception.

Additional reviewed locations: g003/f1: 7d52f1cda4c301e8811acf23263b6ab66ce1930a:.agents/run-all-tests.md:75-103;
g005/f1: 9fcfe774c11b4bb2c57137f859f0e5545dbdc1a4:.agents/run-all-tests.md:75-108.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g001/f1`, `g003/f1`,
`g005/f1`. Original append-only events, revisions, withdrawals and independent judgments are retained in that run's
catalogs. Locations refer to the named reviewed revisions; the original revision context is
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a; persistence previously verified at inclusive tip
ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and frequency were not
measured.
