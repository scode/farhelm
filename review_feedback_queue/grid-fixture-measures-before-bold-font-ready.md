# Grid browser fixture measures bold output without awaiting its font

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The browser layout check does not establish that the bundled bold font is ready before measuring bold output, so it can
assess fallback geometry instead of the intended terminal face.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed missing
readiness; actual test failure frequency unmeasured and unnecessary to establish that missing condition.

Source locations: 7d52f1cda4c301e8811acf23263b6ab66ce1930a:e2e/grid-client/grid-terminal.spec.ts:93,156-183.

test:93 loads only the normal-weight descriptor before mounting at :97-104; its initial static page has no bold content
(:57-69). attach.jsonl:3 supplies BOLD hello. The test feeds it at :156 and captures layout once at :161-177 before
checking the later columns at :180-183. app.css:80-93 registers distinct regular and weight-700 files with font-display
swap; :54-56 documents the two-weight mount prerequisite. terminal.js:460-476 explicitly awaits both descriptors in
production. A computed weight of 700 at test:186 is a requested style, not font-file readiness. The CSS Font Loading
Module Level 3 matching/load algorithms (https://drafts.csswg.org/css-font-loading/#font-face-set-load, displayed source
lines 470-498) confirm load waits only for faces matching the descriptor; unused faces may load later (Introduction and
FontFace status, lines 74-76,162-174). Thus the missing source-level readiness condition is confirmed; no particular
flake frequency or wrong geometry is asserted. Fix: await the regular and bold descriptors before mounting/measuring.

e2e/grid-client/grid-terminal.spec.ts:93 awaits only document.fonts.load('14px "JetBrains Mono"'), then mounts the
component at 97-104. The page before mounting contains no bold content (57-69). app.css:80-93 declares separate
regular/700 faces with font-display: swap; 54-56 documents that terminal mounting must wait for both. attach.jsonl:3
introduces bold hello at column 0; the test feeds it at 156 and snapshots geometry at 161-177, then checks the following
columns at 180-183. The snapshot is one-shot; computed fontWeight >=700 at 186 does not prove that the bold font file
loaded. Production terminal.js:460-476 explicitly loads both descriptors. Fix: await regular and 700 faces before
mounting/measuring, matching that production contract.

Possible coverage and caveats: none established. At inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb, directly
read test:55-245: helper :96 remains regular-only and first layout snapshot :202-218 remains one-shot; app.css:48-94
still registers separate faces. Full SPEC.md Terminal experience :2080-2391 does not accept this test omission;
SPEC_impl.md:1330-1333 requires both mono weights and :616-625 describes font roles, neither covers the test gap.
Same-pin coverage sources were searched again: TODO Planned :23-24 is empty and plans/queue/INDEX.md:1-4 lists no
unlanded plans. The retained font plan, fully read in initial review, targets production terminal.js and
e2e/tests/terminal-font.spec.ts, not this later static-grid helper. BUGS.md has no matching entry. Existing
held-font-test-lacks-mount-attempt-witness.md and TRIAGE_OUTCOMES.md:20391-20404 concern missing mount-attempt
observation, while :13085-13094 concerns mutable constructor options; trigger, consequence and scope differ. No filter
match established. Cross-group synthesis: read g158/f1's full event log. Its overridden-bright-palette routing bug needs
a program palette override, absent from this static attach fixture, and is distinct from f1's font readiness. Other
supplied cross-group findings do not intersect the static-page helper or geometry snapshot.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g167/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7d52f1cda4c301e8811acf23263b6ab66ce1930a.. This
was a source-only review; runtime occurrence and frequency were not measured.
