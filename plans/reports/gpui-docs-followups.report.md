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
