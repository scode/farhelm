# Remove the retired version-query claim from the no-spawn test description

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The no-spawn test says it checks a version-query refusal, but that operation was already removed when the test was
added. Correct the description so readers can tell what the test actually covers. No product behavior or safety defect
is attributed to this wording.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/tmux.rs:9366`. References below use these
paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

At tip tmux.rs9366, the test specification says 'the version query and an input client are refused'. Its complete
body9370-9418 checks the actual listing/session/cleanup/input/wait calls and the final empty invocation log, but
contains no version query or assertion for one. The operation has been retired, so this is a false test-description
claim rather than a missing executable guard.

Exact provenance: exclusive base8a8c9579e04299410652f0e4681975a7198dfc7b contains pub(crate) async fn server_version at
tmux.rs3166; neither this test name nor the quoted phrase exists at base. The sole server_version string-count-changing
commit in base..tip is160592cbf07281ccf145890dca17c6a9989cf40a, earlier than the assigned commit. At introduction
f972458, the new test description9489 already contains the quoted claim; the complete test9493-9542 already lacks a
version query, and a pinned symbol search finds no server_version helper there. The phrase survives at tip9366. Thus the
source-test description was introduced stale, not merely made stale by a later unreviewed operation.

Fix direction: remove only the retired version-query item from this prose list, retaining the valid input-client and
other tested claims. No runtime experiment or code behavior change is needed. SPEC_impl.md Terminal substrate: private
tmux server2818-2831 supplies the intended no-command-after-known-absence behavior; the test's existing invocation
oracle supports its actual calls and is not alleged broken by this documentation issue.

Suggested bucket other; confidence confirmed from pinned base, introduction, tip and read-only symbol history.
proposed_drop false, as directed by parent. Earlier d4 navigation/recording-value exclusion is explicitly withdrawn as
current advice but preserved in history; recording rules provide no general exclusion of that kind. possible_cover none
for this exact wording; related retained q2/r4/c4 are distinct descriptions/references. No missing behavior guard,
security/work-loss consequence or broader test-coverage claim. This is correction of an already investigated known
candidate, not a new discovery.

Additional safe provenance verification for retained d4, not a new finding. In parent
of160592cbf07281ccf145890dca17c6a9989cf40a, tmux.rs3162-3172 still defines the documented server_version helper; pinned
search at160592 itself has no server_version match. The earlier base/introduction/tip comparison therefore identifies
the exact removal rather than merely assuming the string-count history means deletion. A limited tip search for
ensure_server, has_session_for_terminal_less_delete and server_version across assigned tmux.rs/input.rs found no
additional identifier reference. This does not cover every possible stale phrase or every source file. The test's later
plain-English 'version query' phrase is the exact retained d4 mismatch. other/confirmed/proposed_drop false unchanged;
no missing executable guard or safety consequence, no behavior experiment. Prior unsupported recording-value exclusion
remains preserved but withdrawn as current advice per parent; no forbidden coverage source was consulted.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g032/d4`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
