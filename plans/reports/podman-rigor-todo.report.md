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
