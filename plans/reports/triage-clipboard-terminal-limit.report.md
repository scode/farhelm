### What this was about

A broken or hostile remote supervisor could send very large terminal-output frames before the helm's per-terminal event
queue admitted them. The queue counted events rather than bytes, so one terminal could retain far more memory than
intended and affect the helm serving other hosts.

### Things you should know

The stack keeps clipboard writes bounded, removes the deferred filesystem-fallback review record, and caps each incoming
terminal-output frame at 64 KiB. Frames over the cap use the existing stalled-terminal detach path before queue
admission; the host connection stays available, and frames at the cap are delivered. The 256-event queue therefore
retains at most about 16 MiB of terminal payload per attachment. Supervisors continue sending 32 KiB chunks, preserving
compatibility with existing helms.

### Open questions and possible follow-ups

There are no open questions or approval decisions. For future maintenance, preserve the documented protocol-version and
compatibility constraints if the supervisor chunk size or the shared terminal limit changes.

### The PRs

- Clipboard write queue bound — [#1473](https://github.com/scode/farhelm/pull/1473/changes), draft.
- Deferred filesystem-fallback queue cleanup — [#1474](https://github.com/scode/farhelm/pull/1474/changes), draft.
- Terminal data-frame limit — [#1477](https://github.com/scode/farhelm/pull/1477/changes), draft.

### Checks run, reused and skipped

Ran `cargo fmt --all -- --check`, targeted Clippy for `farhelm-proto`, `farhelm-helm`, and `farhelm-supervisor`, the
isolated test-sleep checker (274 delays, none unannotated), changelog-format validation, `dprint` on changed Markdown,
and `git diff --check`. Recorder-backed nextest runs passed the helm attach-limit regression and the supervisor
chunk-limit contract. PR 1's existing 162-test Node run and PR 2's documentation-only validation were reused from their
review records. Skipped the full workspace, desktop, browser, installer, and release batteries because the final changes
do not create material risk in those areas.

### Review gate outcome

The clipboard write queue PR and the terminal data-frame limit PR each received independent high-effort reviews. The
terminal data-frame reviewers identified the required feedback-file cleanup and a test-ordering race; both were fixed
and the focused tests passed again. The final cold read of the terminal data-frame PR message found it accurate and
ready. The deferred filesystem-fallback cleanup PR required no review under the plan.
