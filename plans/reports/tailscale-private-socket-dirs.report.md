# Private Tailscale SSH socket folders

## What this was about

Farhelm used a predictable temporary socket folder for each Tailscale host. If another account had already created the
folder, Farhelm could refuse to connect. A helm restart could also leave a shared SSH connection running from the
previous process. The plan called for private folders and a way to find old connections when you move or remove a host.

## Things you should know

The operating system assigns each Tailscale destination a short private socket folder. Farhelm records its name
alongside the saved SSH host keys so it can find the connections after a restart. If the folder disappears, the next
connection gets a fresh one. Changing or removing the last Tailscale host for a destination closes the shared
connections Farhelm can identify there, including in the single folder older releases used for that destination.

If SSH fails to close an old connection, the host edit still completes and the connection can stay alive until its
normal idle timeout. Farhelm keeps the surviving folder's name so a later cleanup can find it. A replacement host can
register while exits are in progress. Its connection waits to prepare its SSH files, and cleanup preserves saved SSH
host keys if a replacement has registered the destination.

## Open questions and possible follow-ups

An SSH check that has already prepared its command can finish against the old address or report a connection error while
it closes. Farhelm accepts this limit so a pending check cannot hold up Remove. The host list discards any late result
from the former entry or address. A fresh check uses the currently registered address.

## Pull requests

- Allocate private Tailscale socket folders. [#4957](https://github.com/scode/farhelm/pull/4957/changes)
- Retire old connections on retarget and Remove. [#4962](https://github.com/scode/farhelm/pull/4962/changes)

## Checks

- On the final tree `bfe2248c`, pinned nextest passed all 98 selected tests (`76ec6fd4-075d-4390-bc32-1c442f44e735`).
  `cargo nextest run -p farhelm-helm --lib -E 'test(tailscale) | test(/^ssh::/) | test(/^transport::/) | test(/^hosts::/) | test(remove_host) | test(a_confirmed_uninstall_runs_in_order_and_forgets_the_host)'`
  covered lookup and retirement after restart, the macOS socket-path budget, both host edits with and without a
  replacement, publication waiting for retirement and preserving the old connection when a retarget write fails.
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- Static checks passed with `cargo fmt --all -- --check`, `dprint check` on the changed Markdown,
  `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py` and `git diff --check`. The
  last source edit qualified a test-only type and passed in the final test and lint run. The only later edit was
  formatted spec wording.
- Reused evidence for the first draft from `1264618b`: pinned
  `cargo nextest run -p farhelm-helm --lib -E "test(tailscale)"` passed all 13 selected tests
  (`514aa16d-27b3-4a6e-a113-484cee9785d4`) and all-target helm Clippy passed. The allocation patch stayed identical
  through the rebase. The trust and onboarding changes on main affect an adjacent host fixture and independent setup
  behavior. The final test run also covers the rebased stack.
- During development the new failed-retarget test initially failed to compile on `afd2c4cd`
  (`38506a85-dc7e-4f22-ae71-2142aae65028`). The repaired fixture passed alone at `a806e224`
  (`3c641cd7-d79e-468a-88d0-0eb490acd385`) before the final selection passed.
- Skipped the workspace test battery and a live Tailscale node. The targeted tests used private state, local Unix
  sockets and stand-in processes to verify path allocation, exact SSH command arguments and host edit ordering.

## Review

The independent Opus 5.5 source review passed. It caught a race when another host takes the destination and an error
path that could hide a registered host. The follow-up found no unresolved correctness issues. A separate scope review
confirmed that the destination gate preserves Remove while a sign-in is pending. The rare registry-refresh failure
reuses the existing actor hold and retry behavior and was checked in source.

### Landing

#### What landed with it

Three plans landed this round, in this order: the custom agents triage (28 PRs and the landing fix #4969), the agent
catalog and drift test cleanups (#4945, #4951, #4954, #4956, #4959), and this one through its two PRs. This plan changes
only how the helm keeps its Tailscale SSH connection sockets and retires them when a host's destination changes or the
host is removed; nothing else that landed this round or since its base touches that, and the landing's review found no
interaction. Its rebase conflicted only where conflicts below it were carried up, which were resolved there; every file
of this plan's own change is exactly what the executor pushed.

#### Fixes made while landing

- None.

#### Checks

This round landed three plans in queue order: the custom agents triage, the agent catalog and drift test cleanups, and
private Tailscale SSH socket folders. All the checks below ran once on the three plans' PRs stacked in that order on
main as it stood at the start of the landing (`f6ec0f51`), with the conflict resolutions described in the Landing
section of the custom agents triage report; the stack's top commit was `30ae8ef5`. Worker runs were on a separate arm64
Linux test worker; the rest ran locally on x86_64 Linux. The only later change is the landing fix
[#4969](https://github.com/scode/farhelm/pull/4969/changes), inserted after the custom agents plan's last PR, which adds
24 screenshot scene list entries and reformats two triage ledgers and one call in the launcher's golden scene setup;
after it I ran the screenshot batch tool's test (recorder run `18834801-a217-426a-aea5-61f735104753`),
`cargo fmt --all -- --check`, `dprint check` and the changelog check again, and all passed. That final tree is what main
now holds outside the plans queue, apart from what main gained while the custom agents plan was merging: two TODO.md
entries, a SPEC_impl.md note and new review queue findings, none of which touches code the checks covered.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `40997676-1b25-42dd-a283-44987367a22e`): all 8701 tests passed, with no runtime skip. The plans changed the launcher,
  Settings, templates, the quick switcher and Restart in the GUI, the helm's custom agent registry, templates and
  launches, the CLI, the supervisor's Goose restart, the drift tests, and the helm's Tailscale SSH connections; the
  whole suite was the narrowest run that covered them and their callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `c1000071-3994-42e2-ad73-d85b8866e537`): all 184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `9e8e8c24-edac-4e59-9429-c8ab9f3211cb`): 50 steps passed, one skipped by design on Linux (paste).
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `4fc1cb5d-72d7-4edd-870c-909f63840e93`): passed, including the custom agents plan's 24 new scenes and main's sandbox
  scenes whose setup the landing merged with the custom agents plan's.
- The screenshot batch tool's test, `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed on the stack
  (recorder run `8e6013bf-03ee-48d3-9544-f2e4cb90d9eb`) because the 24 new scenes were not in its scene list, and passed
  with the landing fix.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. Two docs
  pages changed (Start a session, and Stop, restart and resume), both in the agent catalog plan.
- `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; one warning, about an
  old citation in the triage archive, already on main) and `python -B scripts/check-test-sleeps.py`, locally: passed.
  `cargo fmt --all -- --check` and `dprint check`, locally: failed on the stack as described in the custom agents triage
  report's Landing section, and passed with the landing fix.
- `bash -n scripts/test-install-sh.sh` and `shellcheck scripts/install.sh scripts/test-install-sh.sh`, locally: passed.
  The custom agents plan added cases to the installer test.

Reused: the custom agents plan's run of the installer test,
`bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` (recorder run `242fd8fd`, at the executor's revision
`cb9cef98`), which covers the cases the plan added for a custom conversation report while an update holds the installed
version. Nothing it exercises changed since: main changed neither the installer nor the CLI since the custom agents
plan's base; that plan's later CLI change is in template flags, and the agent catalog plan's only CLI change is how
`--effort` reads its value, neither of which the installer test's conversation report path uses. Also reused: the agent
catalog plan's live Goose named Create and Restart drift run at its executor's revision (recorder run `5e4a1a9a`), which
spends vendor turns and was not repeated; nothing that landed since that plan's base touches Goose's support code. The
executors' other runs were focused selections that the runs above cover again.

Skipped: real agent and drift runs, which spend vendor turns and run only when asked; a real Tailscale network, which
the tests replace with stand-ins; the workspace doctests, since no documentation example changed; a Mac run, so the
macOS limit on socket path length (which the Tailscale plan's socket folder names are sized for) and the macOS installer
are covered only by tests that run on Linux; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **Saving a new destination for a host, or removing it, now waits for its old shared SSH connections to close.** The
  desktop app's Save or Remove finishes only after that; each close is capped at a few seconds, and a close that fails
  does not block the edit. The code comments on the destination change don't mention the wait, and the order in which
  the new code takes its two internal locks is documented in only one code comment, though every current caller follows
  it. A sentence in the other comments that touch these locks would keep a later change from breaking that order; say if
  you want a TODO entry.
