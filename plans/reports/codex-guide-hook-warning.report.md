### What this was about

The Codex guide called its startup hook warning harmless. Farhelm uses a hook-trust bypass to receive conversation
updates, and that bypass also lets configured hooks, including a trusted workspace's hooks, run without Codex review.
The maintainer chose to correct the guide. It now names the warning, states that consequence, and links to the existing
explanation and instructions for turning hook injection off.

### Things you should know

This changes documentation only. The accepted Codex integration, permission behavior, and hook-injection page are
unchanged. There is no new claim of an exploit or restriction on an informed user's choice to accept the bypass. The
website has not been deployed; it deploys only on request.

### Open questions and possible follow-ups

None required for this plan.

### The PRs

- [#2091: explain Codex's hook-trust warning](https://github.com/scode/farhelm/pull/2091/changes) — draft; guide
  correction and completed triage bookkeeping.

### Checks run, reused and skipped

- Ran targeted `dprint check` on the guide and changed bookkeeping, including after adding the PR link: passed.
- Ran `bun install --frozen-lockfile` and `bun run build` from `website`: installation changed no files; the final build
  produced 32 pages and validated every internal link. The build's i18n/404 content warnings did not fail it.
- Reused the final build after the ledger-only PR-link update. Main's intervening changes were queue/report bookkeeping
  and an unrelated TODO about wrapped terminal links; none changed this guide or its link targets. No rebase was needed.
- Skipped product runtime and browser tests: this documentation change affects no executable behavior. No deployment or
  preview server was started, per the unattended website rules.

### The review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no issues. Its findings were read and accepted; the
executing session separately inspected the diff, formatting evidence and final build. A valid fresh wording cold read
also passed. An earlier wording read opened the diff too soon and was replaced; it is not counted as evidence.
