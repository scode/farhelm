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
