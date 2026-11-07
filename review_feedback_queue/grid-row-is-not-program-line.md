# A rendered row can masquerade as a completed program line

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The new terminal transcript can make a partially rendered program line appear complete to a test. Agent checks can then
reject correct behavior; one direct-signal caller could instead use a process-number prefix and terminate unrelated
work. The exact short-line split, unrelated signal target and regression attribution remain unverified after
investigation.

## Details

Source coordinates from the original records:
`crates/farhelm/tests/e2e/harness.rs:3018-3036; crates/farhelm/tests/e2e/session_lifecycle.rs:5608-5634`;
`crates/farhelm/tests/e2e/codex_identity.rs:35-54,254-260`; `crates/farhelm/tests/e2e/session_lifecycle.rs:5614-5629`;
`crates/farhelm/tests/e2e/harness.rs:3024-3036; crates/farhelm/tests/e2e/session_lifecycle.rs:5614-5629`. References
below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **other**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

complete_marker at 35-40 treats any newline after a marker prefix as proof of completion. wait_for_marker_line at 45-54
accepts that predicate, and assert_resident_launcher at 254-260 immediately parses both PIDs and requires final field
codex. The fixture_marker_waits_for_its_complete_line unit test at 1676-1691 tests raw byte prefixes only. In a grid
stream, harness::grid_text at 177-188 unconditionally appends a newline to EACH rendered screen row, regardless of
whether the program emitted LF. wait_until at 2711-2723 applies the predicate after each such appended data event. For
example a row containing CODEX-LAUNCHER:123:456:co is flattened to that text plus newline; complete_marker returns
123:456:co, and the suffix assertion fails instead of waiting for codex. An earlier cut in a PID or before a colon can
instead panic during field parsing. This conditional transform/predicate mismatch follows directly from source and does
not need a repeated old row, long wrapped marker or changed conversation.

Accessible premises investigated: the actual resident launcher writes the marker with println! then flush at
fixtures/fake_agent/codex_conversation.rs:68-74; root READY and CLEAR-PENDING use writeln! followed by flush at 105-116
and 130-141,172. The terminal host reads arbitrary available PTY chunks and journals each libc::read result without a
line-completion fence at terminal-host/src/host.rs:503-527. The supervisor feeds output chunks at
hosts/terminal.rs:3304-3378; grid_stream.rs:720-793 samples changed rows, and SPEC_impl.md Terminal host / Grid streams
at 2622-2632 promises current screen updates, not newline-delimited program records. Existing harness documentation at
159-174 explicitly describes row versions, not a byte transcript. These contracts do not make a row's synthetic newline
a valid program-LF witness.

Exact missing premise: a source/current-fixture scheduling cut in which one of these short markers is rendered and
delivered with its prefix present but required fields or codex suffix incomplete. The fixture uses normal short
formatted writes, and no trace establishes that they split at a consequential point rather than becoming visible whole.
The local toolchain source lookup for std line-buffer/write behavior found no installed source at the inspected 1.99.1
path; I did not infer multi-write behavior from println!, fetch dependencies, or run a runtime experiment. A retained
frame trace or a controlled partial-marker grid fixture would settle occurrence. Source proves the conditional failure
and missing program-completion boundary, not a real failure rate.

Fix direction: bind the parse to an explicit unique record terminator visible in the row and validate the complete
expected field shape, or obtain the structured fixture values from an independent owned record. Merely waiting for
grid_text's newline is insufficient. Suggested bucket other, maintainer-test reliability only; could be highest no.
Confidence likely consequence with the exact partial-row scheduling premise unverified after investigation; confirmed
predicate mismatch. proposed_drop false. possible_cover g045/g045-l3 is related coordinate-free grid observation but
concerns concatenated wrapped hex rows, and g042/g042-l7 concerns repeated old compact markers. Treat this as new
consumer evidence; commissioner decides whether to merge the broader observer root cause rather than queue a duplicate.

Discovery completion for d3, same issue and consequence; reviewed f42178f9402ce22c60530360edc10d6721ef5b16 through
pinned tip 407a49435278844248d573083fdea6e23fd96c02. Own parent reads now verify that
complete_marker/wait_for_marker_line were identical at F^ codex_identity.rs:35-54 and
resident_codex_launcher_captures_and_resumes_its_child used ordinary harness at F^:319-321. F^ core.rs:1269-1288 gives
SupervisorSeams::default new_terminals=Tmux. Thus converting the ordinary in-process harness to hosts newly feeds
row-synthetic newlines to the inherited raw-line-completion predicate; it is not an inherited grid observer. The
byte-stream helper's incomplete-prefix unit test is no proof of its correctness over grid row snapshots. This lineage
check does not establish that the short actual fixture marker splits across delivered frames. Keep the prior exact
missing scheduling premise and no-runtime-observation caveat unchanged. Suggested bucket other, confidence likely with
occurrence unverified after source investigation; no highest consequence. proposed_drop false. possible_cover related
g045/g045-l3 coordinate-free grid observer / g042/g042-l7 repeat predicate, not an independently verified new product
bug. Commissioner may merge consumer evidence into a broader observer finding. No unresolved source search remains;
resolving actual occurrence requires a controlled partial-grid fixture or retained trace.

Named-input synthesis before a further discovery pass, reviewed tip 407a49435278844248d573083fdea6e23fd96c02. Read
complete g045/g045-e2 history, including its source route, short locked writeln emitter, transport checks, base
comparison and confidence limits. Merge this as additional caller/consequence evidence for the partial-row observer
family, not a new local root. The peer shows first-marker digit-prefix parsing feeds direct SIGKILL after READY without
an identity fence; later complete row versions do not supersede the first prefix in the append-only observer. Preserve
the exact potential consequence: if a partial positive SELF-PID prefix is retained and names an unrelated signalable
same-user process, the test could kill user-owned work outside its fixture. Neither the actual cut inside the short PID
write, the consequential frame delivery, nor such an unrelated target is established. The base byte/snapshot path
already allowed a related possible prefix hazard, so newly introduced regression attribution is also unverified after
investigation. Do not assert an observed signal or failure. The previous d3 phrase 'could be highest no' applies only to
its original Codex suffix/assertion caller; it cannot dispose this new potential wrong-target consequence. Suggested
bucket other for the established observer trust gap; retain potential highest consequence for independent commissioner
judgment, and re-bucket if the external-work route is established. No proposed drop of the signal consequence,
proposed_drop false. possible_cover g045/g045-e2, merge caller/consequence into known partial-row root as commissioner
decides. This is distinct from g041-d1's omitted inter-run spaces and l2's missing successor exit barrier. Peer
contracts: SPEC_impl.md Testing:7543-7547 permits signals to a PID recorded within the run, not an unverified integer
prefix; AGENTS.md Sharing the machine requires leaving others' processes alone. Own exact-source corroboration follows;
no new discovery claimed by this handoff.

Own bounded source corroboration for the immediately preceding named-input synthesis, at tip
407a49435278844248d573083fdea6e23fd96c02. extract_pid finds the FIRST marker and parses consecutive digits without a
required terminator. The caller waits READY then parses SELF-PID at5614-5615 and passes it directly to
libc::kill(SIGKILL) at5629. Its explicit pre-action liveness assertion at5618-5622 concerns daemon_pid, not self_pid.
The safety comment says self_pid is real/current, but contains no identity comparison. SPEC_impl.md Testing7488-7557,
especially7543-7547, was reread in full: recorded-in-run PID reuse is accepted; malformed prefix attribution is not
established as accepted by that text. Preserve the peer's investigated uncertainties: a consequential partial row during
this short fixed write, an unrelated signalable target, and regression attribution versus the prior snapshot route all
remain unverified. The original suffix-parser consequence remains test-only other; this added conditional external-work
consequence remains explicitly retained for independent judgment rather than silently dropped or classified as
impossible. Suggested bucket other for what is established, potential highest as in prior revision. proposed_drop false;
possible_cover g045/g045-e2 same caller/consequence. No new independent finding and no experiment. Initial pinned grep
of the original seven files found no direct libc::kill/marker-PID signal caller there; the named signal test is peer
interface evidence, not an expansion of own mandatory scope.

### Additional consequence: A partial process-number observation can become a test's signal target

Unverified after investigation: if this daemon-cleanup fixture receives an earlier grid row containing a proper prefix
of its agent's process number, it keeps that prefix even after the complete row and READY arrive, and sends SIGKILL to
the prefix. If that positive number names a different same-user process, the test could kill work outside its fixture.
Neither the partial-row schedule nor an incorrect signal target was observed or established for this short fixed
fixture. This extends a known observer root with a distinct signal consequence; it is not a claimed new independent root
or a reproduced failure.

Source route: harness.rs:151-155,177-188 appends every row version with a synthetic newline. :3024-3036 finds the FIRST
marker and parses only consecutive digits; a later complete replacement does not supersede an earlier nonempty digit
prefix. session_lifecycle.rs:5614 waits for READY, then :5615 parses SELF-PID. :5616-5622 verifies a different
daemon_pid from a file, not self_pid's identity. :5626-5629 trusts the parsed PID and directly invokes
libc::kill(SIGKILL); no identity fence or expected-agent comparison intervenes. The other direct signal at :6241-6245
obtains agent_pid from runtime, so it is not another instance of this partial-text route.

Accessible scheduling checks: fake_agent.rs:2859-2903 locks stdout and emits the short SELF-PID line with writeln!, then
READY, then flush. terminal-host/src/host.rs:505-527 journals each nonblocking PTY read, not complete program lines.
hosts/terminal.rs:3306-3385 feeds records and publishes after each; grid_output.rs:403-451 sends changed whole rows and
retains no program-line completeness condition. These facts support the conditional transformation, but source does not
show that this short formatted PID write is split inside its digits, that a grid reader captures that intermediate state
before replacement, or that the truncated PID names a signalable unrelated process. A source-supported large-read path
that can split arbitrary output is not proof of that exact short-write schedule. No runtime experiments were permitted.
Settling it requires a retained trace/deterministic production-equivalent read cut for this fixture plus target identity
evidence; an abstract crafted grid message alone would establish only the already-supported decoder conditional.

Pre-base provenance and attribution: exclusive base 8a8c9579e04299410652f0e4681975a7198dfc7b already has identical
READY/first-PID/direct-kill sequence at session_lifecycle.rs:5597-5615 and the same extractor at harness.rs:2123+. It
used byte attachments (harness.rs:189-199), whereas the conversion routes host attachments through row-version
transcripts. Base tmux.rs:4965-4984 also normalizes snapshot row newlines and :1579-1582 restores modes/cursor after
snapshot content, so a partial initial tmux snapshot may already have exposed a similar first-prefix hazard; an actual
pre-base snapshot cut is also unobserved. Thus do not claim the unsafe signal statement or every possible partial
observation was newly introduced. The newly changed live-row representation adds the known g040/d3-style conditional
exposure, but exact regression attribution remains investigated uncertainty. Pre-base existence alone is not used as a
silent drop.

Contracts: SPEC_impl.md, Testing (:7488-7557), permits signalling a PID recorded earlier within one run (:7543-7547); it
does not establish that a parsed digit prefix belongs to the run. Root AGENTS.md, Sharing the machine with other agents,
requires leaving processes not started by the agent alone. The test-authoring rule requires an owned live fixture before
relying on it. SPEC.md, Terminal experience (:2428-2434), promises changing screen rows, not program-line framing.

Suggested bucket: other for the established test-observer trust gap, with potential highest consequence retained for
independent commissioner judgment. It would move to highest if the truncated-PID-to-unrelated-user-process route is
established; current source does not establish that external-work harm. Confidence: likely conditional defect,
explicitly unverified after investigation for the exact scheduling/target and new-regression premises. proposed_drop:
false. possible_cover: g040/d3 for partial grid/program-line observer family; its recorded consequence did not include
this direct SIGKILL caller. Consider merging as an additional consequence/caller rather than counting another
independent root. No unresolved uninvestigated lead remains.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g040/d3`, `g046/d3`,
`g045/g045-e2`, `g046/g045-e2`. The retained append-only histories preserve all earlier claims and corrections. Reviewed
source references name their own revisions; the inclusive reviewed tip is the hash above. This review used source
inspection only, with no product runtime experiments or live-install changes.
