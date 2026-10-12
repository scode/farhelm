# Replace retries and omitted-paste notes

## What this was about

Retrying Replace after a lost reply could start a second agent in the same folder. Both the session header and sidebar
now keep the unresolved request's identity, including through a YOLO confirmation, until the helm gives a final answer
or the user abandons that intent. An older row's reply or cancellation cannot clear a newer row's unresolved retry.

A mixed paste can silently leave out a genuine fresh `image.png` because it looks like the clipboard's rendering of
another file. The triage decision kept that classification and requested a visible reason. The terminal now names each
omitted image and says it looked like the clipboard's copy of another pasted file. Upload selection is unchanged.

The third outcome concerned a false uninstall-success notice when another GUI removes the host while this window's
uninstall fails. Its small-change gate tripped; it is returned pending, with its original decision and feedback
retained.

## Things you should know

The omission note also appears during an ordinary image-file paste when the clipboard supplies both the file and its
rendering. This is intentional under the requested always-visible note; it can be noisy even when the desired file was
uploaded. The explanation describes the heuristic, without claiming the omitted image is a proven duplicate.

The retry tests prove request-key equality after an aborted first request, then a fresh key after an answered refusal.
They also cover overlapping rows, cancellation, and confirmation continuations. The aborted request does not reach the
helm, so these browser tests alone do not prove recovery after a replacement was actually created and its reply lost.
The existing helm idempotency behavior was reused without changes.

## Open questions and possible follow-ups

For uninstall, even ordinary success removes the host and its progress result before this window can reliably observe
completion. An accepted request is not proof that uninstall finished. A UI-only observed-success flag would therefore
turn normal successful uninstalls into "unknown" too, changing the required normal-success wording.

The realistic dispositions are to accept the remaining two-GUI false-success risk (recommended under the existing
one-GUI complexity policy), hedge every uninstall notice (including normal success), or separately authorize a final
result tied to that uninstall which remains readable after the host is forgotten. That last option changes the helm's
result lifecycle beyond this plan's small notice-logic gate. No uninstall product change was made.

Suppressing ordinary copied-image omission notes would be a future change to the explicit note requirement; this PR does
not silently make that choice.

## The PRs

- [#1936](https://github.com/scode/farhelm/pull/1936/changes) — preserve unresolved Replace retries and disclose omitted
  images; uninstall returned pending. One draft PR.

## Checks run, reused and skipped

- `cargo fmt --all -- --check`, corrected UI all-targets Clippy, and `cargo check -p farhelm-ui --features desktop`
  passed. They cover the changed Rust and desktop renderer compilation. Earlier workspace all-targets Clippy on the
  pre-correction patch based on `f550594a` is reused for unaffected packages: subsequent corrections touch only the UI,
  and the main changes through `fc7ea194` add separate browser tests and bookkeeping.
- Recorded nextest selection for attachment policy and Replace refusal retention passed 11 tests, four slots and zero
  retries, run `f51831ad-0815-4d30-812f-68a8b7d756c5`. Recorded UI JavaScript harness passed 238 tests with no skips,
  run `b8f4fd73-5a98-488f-9128-d02302cd1409`. These cover the pure policy and shipped asset logic.
- CLI and release web builds passed. Recorded browser selection across Replace, paste and YOLO-confirmation specs passed
  30 tests, 15 each on Chromium and WebKit, one worker and zero retries, run `cfc61c39-2728-42be-b81e-1ac945fc70c4`.
  There were no skipped, flaky or interrupted cases. These runs used the final product/test patch based on `fc7ea194`;
  later ledger URL and message edits do not change the tested behavior.
- Browser setup failures are retained: `d9692a1b-fd81-45b4-a654-6564fc967508` could not find cached executables;
  `6b23e44c-fd2a-40b1-9c3c-cddac9fa073c` lacked Chromium libraries; `80c9273e-663b-4138-ad2c-6f4ba169ea00` passed
  Chromium but could not launch the cached WebKit distribution. Correcting the disposable sandbox's mount and runtime
  resolved these prelaunch failures. Recorded single-case Chromium and WebKit probes passed before the final selection.
  An earlier recorder ownership refusal happened before any test or run UUID. These are environment setup observations,
  not latent flakes or behavioral passes.
- Markdown formatting, changelog format and the source-only test-sleep checker passed; the latter found 295 annotated
  delays and zero missing reasons. These inspect changed documentation and test source.
- No full Rust or browser suite: targeted execution covers the changed policy, retry lifecycle and paste integration.
  Uninstall execution was skipped because its product and tests are unchanged under the gate return. Remote SSH was
  unavailable in the browser sandbox; the selected cases use the local host, so no remote-host coverage is claimed.
  Fresh-main inspection found no further changes since the careful rebase; runtime evidence remains applicable.

## The review gate's outcome

The prescribed fresh-context gpt-6.1-sol/high reviewer accepted the final change after corrections to sidebar ownership,
HTTP refusal origin, Replace-with cancellation and test-fixture assumptions. The scope checkpoint found no unnecessary
product mechanism and accepted returning uninstall pending. No finding was left unaddressed. The reviewer ran no tests;
the executor independently completed the checks above. Implementation and validation stayed with the executor, with only
the prescribed scope, acceptance and cold-reader roles delegated. Native tools did not expose exact model attribution or
token usage; private orchestration evidence records that gap. Commit/PR wording and this report also received their
required cold reads before delivery.

### Landing

Landed on 2026-10-12 (UTC) as #1936. The rebase met no conflicts.

#### A fix made while landing

In #1936, before it merged: the Replace retry kept its key only when the browser's own request failed, and treated every
error reply from the helm as settled. But the helm forwards the create to the host, and when that hop loses its reply
the helm answers with an error that deliberately carries no "definitely unaccepted" marker, because the replacement may
already exist; the same holds for "created X but could not remove the original", where the helm supports finishing the
job with the same key. Dropping the key there would start a second agent on retry, which the agreed outcome rules out. A
Replace refusal now ends the intent only when it is a consumed sign-in refusal or the helm marks the create definitely
unaccepted, the rule the launcher's create already follows; keeping a key whose create never ran is harmless, since the
helm treats an unknown key as new. The unit test covers the marker, the replace spec's deliberate refusals now carry it,
and the changelog fragment now says a retry no longer starts a second agent rather than that it always recovers the
replacement.

#### Review before merging

Otherwise no findings. The omitted-image note is drawn as plain text, has no live region, and changes nothing about what
is uploaded or pasted; the terminal change only adds the original file name to paste entries. Notes left as they are:

- The note also appears on ordinary image-file pastes when the copy puts the picture on the clipboard as well, which on
  a Mac is likely the most common image paste; the report raises this as an open question.
- The sidebar keeps one retry slot, so opening Replace on another row between a lost reply and its retry starts a new
  key for the first row.
- The unnamed-clipboard-image wording is no longer covered by a browser test, and the first Replace retry test's cleanup
  stops at the first failure.
- The uninstall item from this plan stays pending, with its finding in the review queue.

#### Checks

- Run now, after the fix: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, the test-delay checker (295
  delays, none without a reason), `cargo clippy --all-targets -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop`; the UI unit tests through the recorder (run `8e93c1c0`, 457 of 457);
  the UI JavaScript tests (238 of 238); and on Chromium and WebKit with one worker and no retries the replace,
  terminal-attachments, header, terminal-create-idempotency and terminal-links specs (run `7f0b81e8`, 164 passed).
- An earlier start of the same checks was stopped during its UI unit tests to take the fix; it is not counted.
- Between those runs and the merge, main changed only plan bookkeeping.

Nothing in the report above was made untrue by the landing.
