## What this was about

The desktop docs did not yet explain how the launcher remembers setups that created a sandbox or container. They also
did not explain what the launcher tells you when a host is suspended. Start a session now explains that a recent setup
restores the recipe and where it runs, including the container host for a container. Launching with it creates a fresh
sandbox or container. Replace with refuses these setups because it keeps the existing environment. The Tensorlake
sandboxes, Containerized sessions and Launch templates guides link to the explanation.

## Things you should know

Enter on a focused recent setup below the search box attempts the launch. Picking the same setup from search just fills
in the launcher. A quick launch to a suspended Tensorlake host stops before starting a session and names the host in its
advice to choose Resume in Hosts. A stopped container gets that advice when Resume is available.

## Open questions and possible follow-ups

None from this plan.

## PRs

- [#4067](https://github.com/scode/farhelm/pull/4067/changes) updates the four guides and removes the finished TODO
  item.

## Checks run, reused and skipped

- `bun install --frozen-lockfile` and `bun run build` in the website directory passed. The final build rendered 36 pages
  and validated all internal links.
- `dprint check` passed on the changed Markdown files, and `git diff --check` passed. The site build covers the two MDX
  pages outside the formatter's configured file types.
- No earlier test results were needed. Rust and desktop runtime checks were skipped because only docs and TODO changed.

## Review gate

Passed. Opus at high effort drafted the prose. The executor checked it against the launcher and its tests, and GPT-6.2
Sol at high effort independently reviewed it. The review led to clarifications about search results, stopped containers
and the quick-launch refusal. A separate cold read checked the PR title.

### Landing

#### What landed with it

Nothing else landed in this round, and since the PR branched from main, main gained only plan-queue changes. The pages
document behavior that landed just before, with gpui-last-mile-fixes: the launch refusal for a suspended or stopped
host, including its "is stopped" wording, which was added while landing that plan. An independent reviewer checked every
claim the changed pages make against the app's code and SPEC.md, and every new link and anchor. It found them true apart
from the two sentences fixed below, both from its review, and found that nothing still true was dropped. It also
confirmed that "a quick launch to a suspended Tensorlake host" is the right scope: a quick launch can never target an
existing container, because a container always belongs to its session and templates only use hosts no session owns. The
report's "A stopped container gets that advice when Resume is available" is still true, but narrow: a container's
refusal can only show in a Restart with or Replace with launcher that was already open when the container stopped, since
those are the only launchers that aim at an existing container, and they are unavailable once it has.

#### Fixes made while landing

- **The containerized sessions page implied you can start a new session in an existing container.** It said "To start a
  session in a paused or stopped container that offers **Resume**, choose it in **Hosts** first." The desktop app never
  offers an existing container as a place to start a session (the host picker, New, Clone and templates all leave out
  hosts a session owns, and every container host is one). What a user meets on a paused or stopped container is that its
  sessions' Restart and Replace are unavailable. The sentence now says that, and to choose Resume in Hosts first.
- **The Tensorlake sandboxes page's "resume it first" linked to the wrong thing.** The link text promised how to resume,
  but it led to the launch refusal, while how to resume is two paragraphs up on the same page. The link now sits on the
  refusal ("a launch on it is refused until then").

#### Things you should know that the report does not say

- When launches start resuming a suspended host themselves (the TODO entry "A launch onto a suspended host should resume
  it first"), the three new paragraphs about the refusal (on the Start a session, containerized sessions and Tensorlake
  sandboxes pages) need revising with it.
- The report names the review gate's model as GPT-6.2 Sol; the plan named GPT-6.1 Sol and said not to substitute
  another. The previous GPUI plan's report says the maintainer overrode its reviewer to GPT-6.2 Sol, so this is probably
  the same override. No action needed: the landed text was also checked by the landing's own independent review.

#### Checks

The change is documentation only (four docs pages and TODO.md), so no test suite ran, per the repository's rule for
documentation changes; everything ran locally.

On the PR as delivered, and again after the landing fixes: `dprint check`, `python3 scripts/check-agents-md.py` and the
docs website build (`bun run build`, which checks every internal link): passed.

Reused: nothing. Skipped: every runtime suite, since nothing executable changed; and the website preview (unattended
run). Pages changed by the fixes: "Containerized sessions" and "Tensorlake sandboxes".
