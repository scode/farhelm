# Input during Delete can replace the Delete's detach notice

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

Input arriving while a session is being deleted can make its terminal detach with an input-failure notice instead of the
Delete's notice or its cleanup-failure explanation. The landed notice fix misses the reader's direct failure path.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/service/connection.rs:533-560`;
`crates/farhelm-supervisor/src/service/connection.rs:533`. References below use these paths' basenames unless another
path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Revision records precise history and closes the remaining input-send premise. The direct reader path already exists at
exclusive base 8a8c9579e04299410652f0e4681975a7198dfc7b connection.rs:533-560, identical at introduction and tip. This
is not a newly introduced reader regression. The assigned aa62b33 fix introduces delete_over coordination and the
explicit broader guarantee in SPEC_impl.md:2450-2456 at that commit (tip2456-2462), but leaves this pre-existing
competing owner outside it. Commissioner should assess it as incomplete coverage of that landed guarantee, with the
baseline preserved here rather than attributing the entire route to aa62b33.

Delete marks at teardown.rs:321, ends hosts at584-606, then takes attachments at623. Natural forwarder
InputFailed/StreamFailed/TerminalEnded verdicts go through connection.rs:1795-1842,2055-2098 and wait at2073.
Independently, reader backend.send_input errors at502-560 remove the attachment at535 and directly notify terminal input
failed at556-560 without checking deletes_in_progress. Once removed, Delete's later extract_if has no viewer to notify
(teardown.rs:640-646), and the waiting natural arbiter finds nothing.

Normal Delete shutdown supplies the input failure, not an independent fault. hosts/terminal.rs:4148-4151 resolves
in-flight input Unknown; :4189 ends its runtime. queue_input at2304-2322 returns InputRefused::Ended when its sole
runtime receiver is gone. HostAttach's pump sends through that queue (grid_stream.rs:1251-1262), publishes errors/lost
fates and closes its own input queue (1208-1248,1292-1301). A valid later input frame sees the pump failure at1071-1074
or the closed queue at1163-1166. If the reader gets attachments before Delete does, it claims the viewer and its notice
while the delete mark still exists. Multiple ordinary key/text frames during an externally initiated Delete supply that
sequence; SPEC.md Lifecycle operations:1679-1680 expressly permits concurrent CLI/agent actions while a GUI is open.
Host endings are concurrent and awaited as a group (session_hosts.rs:688-773), so another host still ending can also
leave this interval open. No malformed message, hostile actor or two coinciding rare failures is needed.

SPEC_impl.md tip2456-2462 explicitly says every natural verdict including failed input waits, and the viewer hears
session deleted or why Delete failed; only stalled detach is excepted. The direct reader sends the same
terminal-input-failed verdict outside that arbitration. UI terminal.js:3751-3788 holds the first banner while the
initiating Delete runs but cannot correct it after failure; external Delete has no local hold. Impact is the wrong
detach reason/premature notice, not an assertion that user input was safe to deliver during deletion or that
processes/data were lost.

Fix direction: stop accepting further failed input immediately, but perform attachment removal/notice ownership through
a Delete-aware separate arbiter retaining exact attachment identity. Do not wait for Delete while holding attachments.
Suggested regression: valid frames while a marked Delete shuts down the host, pump failure observed before Delete takes
attachments, then assert no competing reader notice and the Delete's own outcome notice.

Suggested bucket: other. Confidence: confirmed conditional source route and incomplete landed guarantee; no runtime
experiment permitted or run. proposed_drop false; possible_cover none in named peer records; direct failure branch is
pre-existing as explicitly documented. Distinct from refuted notice-arbiter deadlock and known tmux
presence/consent/scope defects.

Interface reconciliation this turn: read latest g026/l4 in full. Same reader removal/notice owner and same normal
runtime-close premise; possible_cover now g026/l4 (exact duplicate), not an independent defect. Peer adds explicit
finish input_rx.close at hosts/terminal.rs:4254-4256. Retain other, conditional source-confirmed, no work/process-loss
claim and pre-base direct-branch attribution; no proposed drop or lowered consequence. g031/d1 (gone server incorrectly
Unusable), g032/d2 (one implicit idle server), g033/f8 (negative query-gate prose) and pre-base scope-excluded g037/d1
are known separate mechanisms, excluded from new discovery.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g025/d10`, `g026/l4`,
`g009/g009-i10`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source
references name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection
only, with no product runtime experiments or live-install changes.
