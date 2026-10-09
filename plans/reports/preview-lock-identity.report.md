### What this was about

Stopping a docs preview could stop an unrelated Astro command in the same checkout. A preview that died without cleaning
up, including after a reboot, left a background lock whose process number could later belong to a build, preview or
check. The old directory check accepted that later process. A foreground development server normally replaces the lock
with a foreground one, so the TODO's specific foreground-server example was not the ordinary reachable case.

The maintainer chose to compare the process's start time with the lock's recorded `startedAt`, allowing about two
seconds, and to keep using Astro's own stop command. The script now applies both directory and timing checks at its
shared stop point, covering plain stop, replacement, cleanup and takeover of another checkout's preview port. The
covered TODO entry is removed.

### Things you should know

The tolerance is exactly two seconds, covering whole-second readings and their separation. A background lock with
missing or invalid timing, a newer process, or a timestamp more than two seconds ahead of the current clock is removed
without asking Astro to stop its process. Foreground locks remain untouched. When the checks pass, Astro still owns the
stop. Astro 7.3.5 remains pinned; its source confirms that the server writes the lock after it is listening.

The plan's proposed universal guarantee about backwards clock steps was false. A fresh scope review showed that a clock
step can make a newer process appear older even when the lock is no longer in the future. The implementation follows the
maintainer's chosen timestamp comparison and documents that limit; it does not claim proof of process identity across
arbitrary clock history or short process-number reuse races. The extra future-timestamp refusal catches only a visible
clock inconsistency and can leave a real server running after a clock adjustment.

### Open questions and possible follow-ups

None blocks the agreed change. If protection across arbitrary wall-clock changes is wanted, that needs a separate
process identity design and a maintainer decision; the chosen timestamps alone cannot provide it. The real
background-server start/stop exercise remains unverified because the preview port was occupied, as the plan's validation
instructions allow.

### PRs

[#1732](https://github.com/scode/farhelm/pull/1732/changes) — spare later Astro processes when stopping docs previews.
Draft, pushed at `4e23f6c6a0a9b00fa09e9f9732a36bd542d7b56f`; not marked ready or merged by this executor.

### Checks run, reused and skipped

Ran `sh -n website/scripts/preview.sh`, `shellcheck website/scripts/preview.sh`, `dprint check TODO.md` and
`git diff --check`; all passed, including after rebase. A separate documentation pass covered both changed files.

The recorded private manual exercise used only owned harmless Astro-shaped processes, with child-published readiness and
explicit directory and liveness assertions. The original script actually stopped the owned later process, proving that
the exercise distinguishes the unwanted behavior. Final run `9619ad40-98ff-44a0-8d53-f28fa74f1e55` completed with child
and recorder exit 0, complete source/lifecycle capture and untruncated output. The changed script removed old, invalid,
missing and future background locks while preserving the owned process; it also preserved the foreground lock and
process. This is evidence for those cases, not for stopping a real background server. That positive leg was explicitly
skipped because the port was occupied; no other checkout's preview was stopped.

The first setup attempt, run `d47fa5f0-9f10-4174-8f3a-e7bd6144db5c`, failed when the original script's Astro stop
exceeded the private wrapper's 30-second limit, before the changed-script cases. Its evidence remains retained and its
exact cause is unproven. A narrow Astro stop with no server passed in 4.415 seconds in run
`1cd20368-d45f-4ffd-941d-055a3a2b954f`; the owned baseline then passed in `9ee39130-7a10-4a4f-ba0d-ec278774c7c6` after
increasing only the private baseline bound to 60 seconds and making readiness markers unique. The later passes do not
erase the first observation. No latent-flake entry was added for this private same-session setup failure.

Reused the manual and source-review evidence after rebasing from base `394b2e73` to main `a53157ec`. The recorded dirty
source diff matched the pre-rebase change exactly, and the preview script remains byte-identical after rebase. Every
upstream diff was inspected: repository caching, desktop UTF-8 handling, conversation-warning behavior, their specs and
docs, and planning/TODO bookkeeping. None changes the preview script, Astro dependency or its stop contracts. The shared
TODO edits were preserved. No additional runtime test would cover a newly identified interaction.

Skipped Rust, browser, desktop, installer, provisioning and website-build suites: this changes only preview shell
tooling and removes a TODO entry; those suites do not exercise this guard. No hosted CI or deployment was requested.

### Review gate outcome

The required fresh gpt-6.1-sol high source review found no actionable findings. A fresh scope review corrected the
planner's clock guarantee; a fresh gpt-6.1-sol medium wording cold read passed. The independent resume check reconciled
source, claim, reviews and retained evidence. Reviewers ran no runtime tests. Implementation and investigation stayed
local under no-workhorse mode; only required reviewers and readers were delegated. The harness exposes neither actual
served model identities nor token counters, so exact attribution and usage are unavailable. Private galaxy session
records retain that gap under UUID `eec309e5-9251-4738-a762-93388ad02c77`.
