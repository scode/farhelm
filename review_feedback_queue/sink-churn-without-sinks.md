# Sink-registry churn test now exercises an empty registry

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The test intended to catch memory growth from retired old-terminal clients now passes without creating any of those
clients. A regression that stops pruning the registry would escape this assertion. No actual memory leak in shipped code
is claimed.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/terminal_tabs.rs:3958-3986`. References below
use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

terminal_tabs.rs:3959 still calls harness(), whereas adjacent sink tests explicitly switched to harness_tmux. Four fresh
sessions are created and attached at 3960-3970, detached at 3971, waited absent at 3972, deleted, and the registry
length is asserted <=1 at 3982-3986. With default NewTerminals::Host (core.rs:1358, verified in preceding synthesis),
these fresh agents have only host terminals. handlers.rs:1582-1593 explicitly gives AttachTarget::Host no sink and calls
ensure_session_sink only for Tmux. Consequently this test never reaches the registry insertion/pruning lookup
(terminals.rs:2749-2799, specifically prune_dead_sinks at 2770). session_sink_pid is a read-only lookup returning None
for missing entries (2989-2995); session_sink_registry_len returns the map length without creating or pruning entries
(3006-3007). An empty map satisfies both absence and final bound, even if pruning were removed.

The provenance is concrete: F^ terminal_tabs.rs:3907-3944 has the same test using harness(), and F^ core.rs:1269-1288
defaults that in-process fixture to Tmux. This is therefore an introduced loss of this existing test's mechanism, not a
request for additional broad host coverage. SPEC_impl.md Terminal host / Turning terminal hosts on (2137-2168, full
subsection read in initial pass) explicitly names the session sink as an old-terminal subject, requiring the named tmux
fixture. The current product correctly omits sinks for host terminals; it is the test fixture that is wrong. Switch this
test to harness_tmux and assert a live sink or registry insertion before detaching so its premise cannot disappear
silently again.

Suggested bucket: other, maintainer test-coverage regression only. Confidence: confirmed source-level bypass of the
mechanism; no runtime test, mutation experiment, measured leak or false-pass trace performed. proposed_drop: false.
possible_cover: none among the latest g039-g045 peer records read for this discovery. This is a genuinely new verified
finding in this pass, distinct from g046-f1/f4 and the named peer findings.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g046/g046-d1`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
