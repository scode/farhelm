# The planning system counts a stranger's fork PR named like a plan branch as that plan's own work

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

On the public GitHub repository, anyone can open a pull request from their own fork with a branch named like a plan's
branch (`plan/<slug>/anything`). The planning system's queue tool then lists that stranger's pull request as one of the
plan's own. Unattended executors are told to read, rebase and build on a plan's open pull requests, and the landing tool
refuses to record a landing or an abandonment while one is open. So an outsider can put text and code of their choosing
in front of unattended agents that can push branches and whose stacks are merged to main without the maintainer
reviewing them first. They can also stall landings and abandonments for as long as they like.

## Details

Found by the 2026-10-22 prioritized persistent review, group `218-maint-scripts-6of9` (finding f1).

Mechanism:

- `GitHub.open_pulls` (`scripts/plans-queue.py:759-764`) lists every open PR in the repository and keeps only
  `.head.ref`, the head branch name, not `.head.repo.full_name`. `GitHub.pull` (`:769-771`) also keeps only `head.ref`.
  `merged_pulls` (`:773-780`) searches `head:<prefix>`, which also matches fork branches.
- Every association between a plan and its PRs is a bare prefix test on that branch name:
  - `status` (`:1280-1285`) puts any PR whose ref starts with `plan/<slug>/` into the plan's open PRs, flags a pending
    plan with one as unlanded work from an earlier round (`:1334-1335`), and lists `merged ...` hints from fork PRs
    (`:1314-1332`).
  - `_require_merged` (`:878-897`) refuses `landed` while any such PR is open (`:885-888`), and accepts a `--merged`
    number whose head ref merely has the prefix (`:891-896`).
  - `abandon` (`:1027-1030`) refuses while any such PR is open.
- How the procedures act on the listing: `plans/.agents/executing.md:19-31` counts "a pending plan that has PRs from an
  earlier round" as unlanded work, judged by reading the `gh pr diff` of its open PRs. Step 6 (`:70-77`) says "If the
  plan has open `plan/<slug>/*` PRs (a resume), track those bookmarks, rebase that stack onto `main@origin` ... and
  build on its tip". Step 7 reads "the open PRs" for the resume check. Executors run unattended and push `plan/<slug>/*`
  branches. The landing monitor merges a plan's stack to main without the maintainer reviewing it first
  (`plans/AGENTS.md` intro; `plans/.agents/landing.md:82-117`), and starts a round whenever `status` shows a
  `[complete]` plan with open PRs (`plans/.agents/landing.md:160-168`).

Scenario: an outside account forks the repository, pushes a branch `plan/<pending-slug>/01-x` with its own code and a PR
description aimed at the agent, and opens a PR against main. A fork owner chooses its branch names freely. The next
executor that picks that plan sees an open PR, treats the plan as a resume, and reads the outsider's PR body and diff as
the previous executor's work. It may fetch and build on it. The bookmark does not exist on origin, so whatever the agent
improvises to "track" it, `gh pr checkout` for example, pulls the fork's commits. Its stack then carries those commits
under the project's own plan branches, which the monitor rebases, checks and merges. Separately, a fork PR named for a
`[complete]` or `[landing]` plan makes `landed` and `abandon` refuse with "still has open PRs" indefinitely, and keeps
the monitor starting rounds.

How to verify: read the jq filters at `:762` and `:771`. GitHub's REST pull objects carry `head.repo.full_name` (null
for a deleted fork), separate from the base repository. In a scratch public repository, open a fork PR with head
`plan/<slug>/x` and run `scripts/plans-queue.py --repo OWNER/NAME status`: the PR appears under the plan.

Fix sketch: in `open_pulls`, `pull` and `merged_pulls`, also capture `head.repo.full_name`, and drop (or flag loudly as
foreign) every PR whose head repository is not `self.repo`. `merged_pulls` already re-fetches each PR, so it can check
the head repository there. A unit test with a fake GitHub that returns a cross-repository PR should show `status`,
`landed` and `abandon` ignoring it.

Suggested bucket: the reviewer said high, possibly highest. This is maintainer tooling, which stays out of highest
unless catastrophic. But the trigger needs nothing more than a GitHub account, and the failure mode is an outsider
getting code and instructions in front of unattended agents whose work is merged to main and released, which would be
catastrophic if the monitor's own review missed it. Recorded under highest for that reason; triage settles the bucket.

Confidence: likely. Confirmed from code: nothing in the PR association checks the head repository. Unverified premises:
that the repository accepts fork PRs from outsiders (it is public per `README.md:18`, and AGENTS.md says it is or may
become public), and that an executor would actually fetch and build on a fork's commits rather than stop when
`jj bookmark track plan/<slug>/...@origin` finds nothing. The stall of `landed` and `abandon`, and the agent reading
outsider-written PR text, follow directly from the code and the procedure.

Possible cover: none found. The reviewer searched `review_feedback_queue/INDEX.md`, `TRIAGE_OUTCOMES.md`, `BUGS.md` and
`TODO.md` for fork or plan PR handling; recording found nothing either. SPEC_impl.md "Development tooling assumes a
single-user machine" is about other local accounts, not GitHub users, so it does not apply.
