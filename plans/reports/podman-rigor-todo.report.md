## What this was about

Eight reviews raised concerns about experimental containerized sessions:

- Stop approval cards and menu text describe ending the agent, although Stop also ends independent work in that
  container's terminal tabs.
- The wrapper guide promises that terminal tabs survive Stop, without the container exception.
- Podman's detach keys can leave the agent running inside its container while Farhelm shows it as exited; later Delete
  can end that work without a live-agent warning.
- If the optional GitHub token file is inside the writable checkout, an agent can replace it with a link to a readable
  host file. A later tab open can deliver that file's text into the container as the token.
- Podman can copy host proxy settings, including credentials, into the container without a separate sharing choice.
- If a user replaces the checkout at its saved path, opening a new tab in a stopped, retained container can run old
  shell startup files against the replacement checkout.
- After a crash and reboot during creation, a failed retry can remove a container that already holds agent-created
  files. This affects files inside the container, not the checkout.
- Selecting a checkpoint image for account inspection might restart the image's saved workload and host mounts before
  Farhelm refuses creation. This route was not established for supported use: restoring such images requires root,
  whereas Farhelm requires rootless Podman.

The maintainer chose to track all eight as future Podman work in one Near term TODO entry. This PR makes that move,
removes their review-queue files and index entries, and records the same PR in all eight triage outcomes.

## Things you should know

This changes work tracking only. None of the eight behaviors was fixed or reproduced during execution. The TODO entry
preserves the recorded triggers, uncertainty and proposed directions, including the unusual token-path choice, the rare
crash/reboot/retry combination and the unverified supported-use premise for checkpoint images. The rest of TODO.md is
unchanged.

## Open questions and possible follow-ups

The eight TODO bullets are the agreed follow-up work. Three retain a choice for that later work: whether containers
should receive the host's proxy settings and credentials, with the specification made accurate either way; whether
opening a new container tab after a manual checkout replacement should follow the new folder, as ordinary tabs may; and
whether a container found after an interrupted create should count as evidence that the agent already ran, rather than
letting a failed retry remove its files, or whether that rare loss should be explicitly accepted. No decision is needed
to complete this bookkeeping plan.

## The PRs

- [#3017: move Podman review queue items into TODO](https://github.com/scode/farhelm/pull/3017/changes), draft.

## Checks run, reused and skipped

- Ran `dprint check TODO.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`: passed after the initial edit and again
  after adding the PR URL to all eight Execution fields. These are the three surviving changed Markdown files.
- Reused the independent content review after rebasing over three unrelated queue-claim commits: the reviewed patch was
  unchanged. The later ledger edit only replaced the eight PR placeholders with the draft PR link.
- Skipped runtime tests, builds, CI and the website preview: only TODO and review bookkeeping changed, with no code,
  specification or product-documentation change. No previous runtime result is claimed as validation of this work.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort passed the change with no findings. The independent
commit-message cold read passed after a rewrite made the review-queue source and agreed future-work motivation clear.

### Landing

Landed with no fixes at landing. The eight review findings about experimental containerized sessions now sit in one Near
term TODO entry, "Podman rigor: review findings for containerized sessions", and their review-queue files and index
lines are gone. Each of the eight triage outcomes now names #3017 as where it went.

- #3017: the TODO entry, the eight triage outcomes' Execution lines, and the removed review-queue files.

#### What landed since the stack was based, and whether it interacted

Three changes landed on main after the plan's base and before this round started. Two fixed the tests behind three test
failures earlier landings reported on main: two tests of sessions whose tmux server vanished while the supervisor was
down now expect such a session to list without a terminal (#3012), and a supervisor unit test's fake tmux now answers in
the format the supervisor reads today (#3013). The third recorded a browser flake in FLAKES.md and TODO.md's Deflake
section (#3014). Only #3014 touched a file this plan touches, TODO.md, in a different section, and the plan's entry
neither repeats nor contradicts it. A fresh-context review read the plan against these and agreed. While the landing
ran, an approved planning PR (#3016) also landed, adding plan pointers to other TODO.md entries (pinned session order,
bulk actions, the host attention list, Tensorlake fixes, host filter feedback). It touched none of the lines this plan
adds, so #3017 merged on top of it unchanged, and main differs from the tested tree only by #3016's pointers.

#### Things you should know

- The proxy-settings item asks to make "the spec's" list of what a container receives accurate. The triage assessment
  behind it notes that the implementation spec (SPEC_impl.md) has the same incomplete list, so whoever takes the item up
  should correct both.
- In the review queue's index these eight sat under "Highest priority". The TODO entry keeps each item's trigger and
  caveats but not that label, since the maintainer asked for all eight to go into one Near term entry.

#### Checks

This round landed only this plan, stacked on main `0a50c784`, and it changes only TODO.md, TRIAGE_OUTCOMES.md and the
review queue's files. On the stack:

- `dprint check` over the whole repository and `python3 scripts/check-agents-md.py` (removed Markdown files can break
  cited headings): both passed, the second with no errors and the same three warnings about older text that main already
  had.
- Skipped: every build and runtime test, Rust, JavaScript, desktop, installer and browser alike. The change is ordinary
  Markdown bookkeeping that no build, test or generated artifact reads, which `AGENTS.md`, "Tests and validation", rules
  out running them for.

The executor's report calls #3017 a draft; it landed rebased onto current main with its content unchanged. The eight
triage outcomes also link #3017 as a draft PR and name its branch, which was true when they were written. Nothing else
in the report became untrue at landing.
