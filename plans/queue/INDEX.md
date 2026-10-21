# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 73b85a] `launcher-draft-model.md` — Hold the session launcher's draft in one value whose only mutator applies the reset, in-flight and retry-key rules, fixing the handlers that drifted from them (structural refactor; aligned fixes in their own PRs).
- [in-flight 85164a] `modal-dialog-shell.md` — Render every modal dialog through one shell that owns its Tab trap, page isolation, focus return and scroll reachability, and give every modal dialog isolation and a Tab trap (structural refactor; aligned fixes in their own PRs).
- [landing 0c9279] `launch-attempt-owner.md` — Remove a launch's spec, status and progress files only through one owner with named policies, so create and restart stop drifting on which files they clean up (structural refactor; aligned fixes in their own PRs).
- [complete] `helm-launch-admission.md` — Make the helm's create and restart-with paths accept only launches that went through workspace-trust handling, each route keeping its own check order (structural refactor; aligned fixes in their own PRs).
- [pending] `popover-anchor-close.md` — Route closing anchored popovers on row movement through one owner, and close the three movements left uncovered on purpose (structural refactor; aligned fixes in their own PRs).
