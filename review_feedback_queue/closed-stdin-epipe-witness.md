# Closed-stdin regression does not witness the failed forwarding attempt

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The test for keeping SSH input open after the remote stops accepting it can pass without ever reaching the
failure-handling boundary it claims to check. A refactor that buffers input without forwarding it can leave that
boundary untested while the test stays green. This is a regression-test sensitivity defect, not evidence that current
forwarding is wrong.

## Details

Source coordinates from the original records: `crates/farhelm-helm/src/provisioning/ssh_stand_in.rs:620-631`;
`crates/farhelm-helm/src/provisioning.rs:6717,7263-7268; provisioning/ssh_stand_in.rs:620-631`. References below use
these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

At ssh_stand_in.rs:609-618 the marker proves only that the remote closed its descriptors. The first helm write at
620-623 puts 31 bytes into the helm-facing pipe. The two assert_still_open calls at 624-625 inspect stdout/stderr for
300ms each (helper 530-533). Neither output depends on forward_input: the remote deliberately closed both outputs and
stays alive independently until the release file. The second successful 15-byte write at 628-631 proves the helm pipe
still accepts input, but does not observe that forward_input tried writing the first input to the now-closed remote. The
claim at 626-627 ('By now the master has tried to pass the first write on and found the remote’s stdin closed')
therefore has no named readiness witness.

Concrete alternate wrong implementation, requiring no scheduler starvation: at the start of forward_input, wrap its
ChildStdin in tokio::io::BufWriter::with_capacity(8192, session), retaining the existing loop and adding no flush.
Schedule the forwarder promptly on both inputs. The combined 46 bytes fit in the buffer, so write_all succeeds without
an underlying OS write/EPIPE. This implementation is wrong for the module's streaming-input contract (lines 29-32 and
helper contract 293-299), but every assertion in this particular test still passes: both helm writes succeed, output
stays silent, the remote is released and exits with 4, and Master aborts the input task at 263-264 before reporting the
final client status. Aborting drops the buffer and helm reader, so the final late-write failure at 638-640 also passes.
This example establishes the missing witness under ordinary fair scheduling, rather than hypothesizing that a ready task
never runs.

Pinned dependency source checked: tokio 1.53.2 io/util/buf_writer.rs:23-26 says dropping it discards buffered data;
42-54 sets capacity; 119-134 buffers small writes without calling inner.poll_write unless capacity is exceeded. No
assertion or control signal in the test flushes it or fills it before the second write. Current forward_input remains
correct: 306-319 attempts the remote write and, on failure, holds the helm reader while awaiting client death. The
existing current-code execution normally reaches EPIPE during the silence windows, but that is a fact about the
implementation under test, not a witness supplied by the test.

Focused peer check: g004/progress.md and pinned provisioning.rs:6690-6770,7234-7301 show that the caller regressions use
explicit remote lingering/release/done markers for stderr lifetime and cancellation. The probe's cat can observe EOF
when stdin closes without proving that nonempty bytes were forwarded; the stalled-transfer feeder holds stdin and never
writes. Those particular tests add no EPIPE witness for this stand-in test. Other transfer tests may catch a global
buffering mutation; this review does NOT claim the whole suite passes that mutation, or that the hypothetical refactor
is shipped.

The verbatim .agents/test-authoring.md rules are: 'Assert the fixture premise before the action or measurement that
depends on it. A successful setup command alone does not establish the state the assertion needs. When a wait times out,
report whether the fixture premise still holds.' And: 'Assert an observable that distinguishes the competing mechanisms.
A shared error string, absence of output or elapsed delay alone may not establish the cause the test claims.' The test
should wait for an explicit forwarder-side first failed-write witness before its second helm write, or use an equivalent
observable that forces and proves that boundary; preserve the intentionally live remote and open/silent output
observations.

Suggested bucket: other (test sensitivity; no established product failure or highest consequence). Could be highest: no.
Confidence: confirmed source-level missing witness and checked alternate buffered implementation semantics; mutation was
not executed because experiments are forbidden. Remaining premise: no source premise remains for the local test claim;
whether other tests kill the alternate mutation was not investigated and is not needed or claimed. proposed_drop: false.
possible_cover: none. This supersedes the prior f4 candidate decision that incorrectly required an existing shipped bug.
No separate discovery pass was performed.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g005/f4`, `g004/f4`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
