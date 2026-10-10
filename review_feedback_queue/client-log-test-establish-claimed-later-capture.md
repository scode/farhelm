# Client-log test does not establish its claimed later capture

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The client-log test could pass with later capture permanently disabled.

## Details

F296 — **possible** — `crates/farhelm-ui/js-tests/client-log-shim.test.js:605` — Client-log test does not establish its
claimed later capture

After hostile input throws during capture, the test counts calls to the original console rather than inspecting
forwarded entries. The original console runs before the capture guard, so a guard never reset can still satisfy the
later-call assertion while dropping that entry. No mutation was run and production resets correctly. Require the flushed
normal-after-hostile entry in captured output.

## Evidence and triage context

- crates/farhelm-ui/js-tests/client-log-shim.test.js:587–610 claims that later capture works, but its later assertion
  checks only consoleErrorCalls.length.
- crates/farhelm-ui/js-tests/client-log-shim.test.js:331–335 records original-console calls without inspecting captured
  entries.
- crates/farhelm-ui/assets/client-log-shim.js:225–229 lets the hostile circular object's String conversion throw into
  guardedCapture.
- crates/farhelm-ui/assets/client-log-shim.js:324–337 contains the capture guard and its reset.
- crates/farhelm-ui/assets/client-log-shim.js:451–455 invokes the original console before checking the guard. A stuck
  guard therefore still produces the asserted second original-console call while dropping the later entry.
- No matching acceptance or queue item was found. SPEC_impl.md:2944–2957 describes the forwarding pipeline but does not
  accept an oracle that fails to observe capture.
- Retain as definite correctness, other bucket, would_fix. Observe the flushed normal-after-hostile entry; no runtime
  test or mutation was performed during verification.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_04_cor:p1:C6`.

- `ui_desktop_04_cor:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed:
  not separately tagged in candidate list.
