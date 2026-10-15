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

### Landing

Landed as #2091, the last of the round's four plans, tested stacked with the other three on top of main.

#### Review

An independent fresh-context review confirmed the new sentence is accurate: Codex launches with Farhelm's conversation
hook carry `--dangerously-bypass-hook-trust`, so untrusted hooks in the Codex configuration home run, and a trusted
workspace's hooks run without Codex's per-hook review. The text neither overstates nor understates that, and both links
resolve.

#### Fix made while landing

The paragraph described the hook only as "the commands that tell it which conversation you are in" and had dropped what
the user gets. It now says where the warning appears, that the hook is what lets restart resume the conversation you
were in, and that turning it off means those Codex sessions can't be restarted.

#### Left as is

Follow-up: the hook-injection page's "what the warning means" note covers only hooks in the configuration home, not a
trusted workspace's hooks; the plan left that page alone.

#### Checks

The website build passed with the four plans stacked on main (see claude-trust-decimals); only Markdown changed here.
