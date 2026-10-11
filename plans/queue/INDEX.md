# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 138aeb] `installer-and-font-license-fixes.md` — fix three installer and docs-site bugs: a directory at a file destination, an incomplete latest-version answer, and the font license the UI omits
- [complete] `bell-build-mismatch-fixes.md` — keep the notification bell and the read/unread toggle current while the helm and the page are different builds
- [complete] `launcher-and-terminal-copy-fixes.md` — fix the launcher's retry key and template-list errors, and copy in restored terminal snapshots
- [in-flight 04f40d] `supervisor-lifecycle-test-fixes.md` — make seven supervisor lifecycle tests clean up their fixtures and prove the paths they name
- [pending] `helm-and-portability-test-fixes.md` — make five helm routing, upload, portability and timing tests prove what they claim
- [pending] `e2e-session-test-fixes.md` — make seven end-to-end terminal and session tests prove what they claim
- [pending] `browser-spec-fixes.md` — recover credentials after a failed rotation test, tighten the long-folder sidebar test, and clean up after the real-Claude spawn test
