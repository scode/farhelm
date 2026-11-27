### What this was about

The real-agent drift tests (the opt-in tests that run the actual Claude Code against Farhelm to catch changes in how it
behaves) run Claude with a private copy of its configuration. In that copy Farhelm removes the "trust this folder"
approval for the test's workspace and every folder above it, so a test cannot pass just because you once trusted a
parent folder. At the end of a test run, Farhelm copies some of the files Claude changed back into your real
configuration (a refreshed login, for example).

A review of the earlier change that strips trust raised a risk: if Claude saved that private copy during the run while
your real file stayed unchanged, the copy-back could write it over your real configuration and silently remove your
trust approvals for the parent folders. At planning, the code was checked and cannot do this (the trust files are always
private copies, never among the files copied back, and the copy-back separately refuses to overwrite them); you chose to
add only a regression test that keeps it that way, with no product changes.

What was built: exactly that. One test simulates a save of the private copy during a run (a new file renamed over the
old one, the way Farhelm itself saves the trust approval it adds) while your real file is left alone. Finishing the run
reports nothing and leaves the real file byte for byte as it was, with the parent folder's trust still set. The to-do
item that asked for this check is removed.

### Things you should know

- Nothing in the repository establishes how Claude itself saves that file, so the test models the save on how Farhelm's
  own trust write does it (write beside, then rename). Copy-back does not depend on how the file was saved, only on
  whether the trust file is among the ones it may return, so either way of saving is covered.
- Only one of the two existing protections is exercised here: that the trust files are never among those copied back.
  The other (copy-back refusing to overwrite a protected file) already had its own test.

### Open questions and possible follow-ups

None.

### The PRs

- #5134 `test: pin that a saved claude trust mirror never reaches the real config`: the test and the TODO entry's
  removal.

### Checks

Run on the pushed change (main has moved since only by queue bookkeeping):

- `cargo clippy -p farhelm --all-targets -- -D warnings`: clean.
- Through the test-run recorder (run 2ce3e58c-97b1-4828-9334-4c1aac05571f, 12 passed), the drift tests' private
  configuration tests, which run without Claude or any network:

  ```
  cargo nextest run -p farhelm --test e2e -E 'test(/real_agent_capture::drift::private_config::tests::/)'
  ```

- The test-sleep check (no unannotated delays), `cargo fmt`, and `dprint check` on TODO.md: all clean. The last edit
  after these runs only rewrapped a comment.

Skipped: the real-agent drift tests themselves, which are opt-in and spend real Claude turns; no product code changed.
The run above was narrowed from the plan's whole drift selection to these tests for that reason. No changelog entry,
since the change is test-only.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It found no bugs and confirmed the test would fail if the
trust file were ever returned by copy-back, whether through the test's early check that the private copy really had the
parent's trust removed, or through the final byte comparison. Its findings were two comments that described the save and
the copy-back check inaccurately, a redundant import and a roundabout way of making that early check. All were fixed,
and a verification pass by the same reviewer confirmed them; its remaining wording suggestion was applied too.
