# Dense OSC admission repeats whole-row construction

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

Unverified after investigation: admitting a dense row of program hyperlinks in a container terminal repeats whole-row
construction for each candidate, potentially delaying pointer or UI work. The repeated work is source-confirmed;
material latency has not been measured. This is separate from asynchronous redraw losing a hover.

## Details

Source coordinates from the original records: `crates/farhelm-ui/assets/terminal-files.js:269-276`;
`crates/farhelm-ui/assets/terminal.js:1372`. References below use these paths' basenames unless another path or revision
is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **other**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

terminal-files.js:269-276 snapshots each OSC candidate using a separate this.view.lines() reader. Grid view.lines
creates gridBuffer.linkLines(source) (terminal.js:1202; grid-terminal.js:2028-2029). linkLines creates a new tag map and
linkIdentities cache per call (grid-buffer.js:666-668). Each line(abs) constructs a complete lineView (:679-683), which
iterates every run, serializes every linked run's id/URI through tagOf/linkKey (:627-635,669-676,352-355), and allocates
the whole row's cells. linkRangeText only needs the current candidate's text, but asks that whole-row construction again
per candidate. Hence n separate hyperlink runs on a row with n runs cause n*n run/tag work and repeated URI
serialization before offering any link. This is synchronous work, even for an all-web row where checkoutLinks later has
zero file candidates and immediately returns (:301-307). The 32-file admission cap is applied after oscCandidates has
finished, so it does not bound this cost.

Reachability/bounds inspected: terminal.js:1353-1374 calls oscCandidates for the complete OSC row in container mode.
grid-model.js:60-65 permits 1000 columns, 256 link-table entries and 2083-byte URIs; :224-252 permits one run per
column. Alternating two explicit ids/targets can therefore create n separate one-cell links without exceeding the
link-table cap; grid-buffer.js:389-405 retains the distinct discontiguous runs. Two 2 KB targets fit the 1 MiB message
cap. At n=1000 the source structure requests roughly one million tag-key constructions, each including a ~2 KB target,
for this one signature batch; even n=128 requests 16,384 such constructions. These are operation/allocation counts, not
timing measurements or a claim of simultaneous retained memory. Generated-id chains can additionally repeat backward
discovery because every candidate owns a fresh cache, but that extension is not needed for the quadratic current-row
route.

Governing context: SPEC_impl.md, Terminal widget / grid link identity, :1795-1800 says per-question row caching keeps
resolution linear in runs examined. The separate-reader loop prevents sharing that cache across this admission question,
although each reader individually obeys the bounded resolver. SPEC.md, Terminal experience, requires terminal links to
work on click; no explicit accepted expensive-admission exception was found in the previously read full terminal
contract.

Checks performed: inspected exact caller, candidate loop, text reader, line adapter/cache ownership, row/link decoding
and caps, and rowLinks grouping. Missing premise: measured material responsiveness impact on supported engines/hardware
and whether any later optimization neutralizes the allocation cost; current source shows no such cache in this call
path. Source inspection establishes the repeated work but cannot establish elapsed latency. Runtime/benchmark
experiments are prohibited by this charter, so a browser benchmark with a containerized terminal and dense alternating
OSC runs would settle the latency premise if separately authorized. Suggested fix direction: share one question's reader
and memoize its row views or provide a text-only range reader, avoiding hyperlink-tag serialization for each candidate's
text extraction.

Suggested bucket: other for established redundant bounded work; high would need evidence of material UI stalls. No
established security/data-loss consequence, and no highest claim. Confidence: likely performance defect,
investigated-unverified after investigation because user-visible latency is unmeasured. proposed_drop:false.
possible_cover:none in own or g023 latest logs. New investigated-unverified finding, not a duplicate of g022/l1 or
g023/f1/l4.

Discovery completion check: at introducing commit bed6f95fc56df634556a44b2caca0206921d1dfc, grid-buffer.js:340-342 and
:599-661 already perform the same full lineView construction and per-run linkKey serialization with per-reader maps.
Thus the repeated-row-work route is present at introduction and survives at pinned tip, rather than depending on a later
unrelated change. No additional performance measurement was performed; the material-latency premise remains explicitly
unverified.

Interface handoff for g023, requested by commissioner before the next discovery search: independently inspect the
container provider call terminal.js:1353-1374, which calls FileLinks.oscCandidates on the whole OSC row before
checkoutLinks can apply its file-only 32-candidate cap. Each candidate currently obtains a fresh grid linkLines reader
and constructs all row cells/tags. Confirm cache ownership and alternating explicit-link reachability; identify any
provider-level bound or memoization missed here. This is corroboration of existing g022/l2, not a request to repeat it
as a new issue. Keep latency investigated-unverified unless source supplies a stronger established premise; no runtime
experiment is authorized. Latest g023 records read this turn contain no l2 corroboration yet. Record this question in
the peer handoff; parent routes it without allowing this reviewer to edit g023s owned log.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g022/l2`, `g023/l5`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
