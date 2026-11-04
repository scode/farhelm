# Orderly-restart screen comparison captures before the fixture's final prompt

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Correct terminal recovery can fail the restart regression because its expected screen is captured before the fixture
finishes writing the post-echo prompt.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed unsupported
screen-equality invariant with source-supported false-failure route; occurrence/frequency unmeasured.

Source locations: crates/farhelm/tests/e2e/host_restarts.rs:308-312,352 at ca9b0a19856dc136f63ad2d14f1076027edce1eb.

host_restarts.rs:299 waits only for echo:before-restart, :303 captures screen_tail(4096) before stop_in_order_for_test,
and :343 requires exact equality after replay. In this same commit, crates/farhelm-fixtures/src/fake_agent.rs:1762-1764
writes the echo line and then a separate > prompt. host_terminals.rs:134-151 waits only for substring presence. The
runtime may expose the echo before that later prompt; the stop/replay can correctly include the prompt, making the
equality fail. host_restarts.rs:308-312 explicitly allows additional output during stop, but the screen comparison does
not.

Correct terminal recovery can fail the restart regression because its expected screen is captured before the fixture
finishes writing the post-echo prompt. Original locations: host_restarts.rs:299-303,343 at
a4345e86a3f51f4c5abf3b2e97737991df900142; tip locations: :308-312,352. At the tip, fake_agent.rs:1762-1764 writes a
newline-terminated echo and then separately writes and flushes > . host_terminals.rs:136-153 waits only for substring
presence. The expected screen capture does not wait for that final prompt. Static interface checks:
hosts/terminal.rs:3153-3172 routes each Output separately, :3224-3306 feeds it and calls after_record, :3786-3818
publishes status after individual records. screen_tail reads the emulator view directly (:2139-2146), so no
frame-coalescing or prompt-completion barrier protects this assertion. Local Rust 1.98.1 primary std source docs,
std/io/stdio.rs:617-624, confirms terminal StdoutLock flushes on newline; this is supporting standard-library contract
evidence, not a claim about the exact compiler used for any historical test run. A schedule where the echo is observed
before the subsequent prompt is therefore allowed by the actual source interfaces; no measured flake frequency is
claimed.

Possible coverage and caveats: none found. Synthesis re-searched every required coverage source at the immutable tip
using host_restarts, before-restart, post-echo, screen comparison, safe-to-type, echo/prompt and same-run PID wording.
Full relevant SPEC_impl.md Testing section :6742-6808 and exact same-run policy were read; they do not accept this
test-screen race. TODO.md Planned :23-25 is empty; no matching unlanded plan, BUGS entry, queue item or ledger decision
was found. The rare-self-correcting diagnostic filter does not clearly cover a false regression-test failure, so no
exclusion is proposed.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g153/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is a4345e86a3f51f4c5abf3b2e97737991df900142;
persistence verified at full pinned tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review;
runtime occurrence and frequency were not measured.
