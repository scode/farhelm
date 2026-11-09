## What this was about

Uninstalling Farhelm from a remote Mac could refuse because an unrelated mounted volume had " (" or " on " in its name.
The final directory-removal check could hit this after the supervisor's background job was already gone, leaving
uninstall partly finished. The mount check now follows the maintainer's chosen best-effort policy: a clearly detected
filesystem mounted on or inside the installation still prevents removal; ambiguous or unreadable mount information does
not prevent it.

## Things you should know

This deliberately accepts that a nested mount with an ambiguous name can go undetected, and removal can then delete the
files it exposes. Malformed and non-absolute mount lines are skipped silently; an empty or failed mount read reports no
mounts. Linux keeps its existing mount check and read-failure refusal. The existing ownership and path guards remain.
The original unreleased remote-Mac changelog entry records the caveat, and the completed TODO entry is removed.

## Open questions and possible follow-ups

None required for this plan. No real-Mac run was required or performed; the regression evidence uses controlled shell
tools and the actual removal backend on Linux.

## The PRs

- [#3267 — unblock remote Mac uninstall on mount parse errors](https://github.com/scode/farhelm/pull/3267/changes),
  draft on main, head `940ff2ce08a938f3abb2ad4409e1327e79edec16`.

## Checks run, reused and skipped

Run now: recorded `cargo nextest run -p farhelm-helm --lib` filtered to the Mac mount parser, Mac final removal,
uninstall inspection, uninstall planning, uninstall refusals and host-command removal guards. Recorder run
`8bbf3851-75af-41c0-b5c9-850f96ecf8e8` completed successfully: 24 passed, zero failures or flaky results, 1,639 selected
out. Four shared nextest slots, zero retries, pinned nextest 0.9.148, no tmux needed. The complete retained output has
no runtime `SKIPPED` messages; the new final-removal case first proves that a known nested mount preserves the
installation, then proves that unrelated ambiguous volumes permit removal. Source capture and JUnit evidence are
complete, with no output truncation or forced cleanup.

Also passed: `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, dprint on the changed
Markdown, and the required isolated-interpreter `python -B scripts/check-test-sleeps.py` (429 delays inspected, zero
missing reasons).

Reused: the recorded runtime result covers the executable and test changes in the final PR unchanged. Its source was the
reviewed working copy on main `71dfc8b992f8c357e038b350b18749e720fe2ccd`; the only later source edit moved the changelog
caveat into the original feature's fragment. A fresh fetch found no incoming main changes to reconcile.

Skipped: browser, desktop, real SSH/Mac, the full workspace suite, broad Clippy and CI dispatch. The changed behavior is
an embedded POSIX-shell check, covered directly and through the production removal backend with child-only fixture
tools; those broader checks add no identified coverage for this change. No live installation was changed.

## Review gate's outcome

The required fresh gpt-6.1-sol/high reviewer found no correctness, design, idiomatic-code or test-authoring findings.
The same reviewer also checked the final fragment-only correction and found no issues. The commit/PR wording passed its
separate cold read. Every touched file received a documentation pass; no scope expansion or omitted required behavior
remains.

### Landing

Landed alone in its round. Between the executor basing its PR and the checks below, nothing but planning bookkeeping
reached main, so nothing could interact with it. One change landed while this PR was merging: a fix to the macOS
acceptance test of the desktop app's own uninstall (#3268, which now expects an update from a very old app layout to be
refused). That test drives a local installation, not a remote Mac host, and does not use the mount check this plan
changes.

A fresh-context review read the change against main and found nothing that had to change before merging. Uninstall
planning and the final removal still use the new Mac mount check only on a Mac. The Linux check, which Tensorlake
sandbox cleanup also uses, is unchanged and still refuses when it cannot read the mount table. The reviewer also ran the
new check under dash and under bash in POSIX mode, the shell a Mac runs scripts with, against a Mac-style mount table.
It found the clearly named nested mounts and skipped the ambiguous lines.

#### Fixes made at landing

- Two code comments on uninstall's inspection still said an unreadable mount table always fails it. That is now true
  only on Linux; the comments now say the Mac check is best effort and link to it.
- The hosts documentation page (Using, "Manage hosts") said uninstall refuses whenever something is mounted on or inside
  the installation directory. It now adds that on a Mac this check is best effort, and that a mount whose name contains
  " (" or " on " can go unnoticed. That is the same caveat the changelog entry gives. The page does not mention the
  rarer case of a Mac whose mount list cannot be read at all, which uninstall now also treats as having no mounts.

#### Checks

Run here on the plan rebased onto main as it stood before #3268 merged, with the landing fixes. That is the tree that
merged apart from #3268's test-script change.

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`, the test-sleep checker
  and `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- Recorded `cargo nextest run -p farhelm-helm --lib -E 'test(/provisioning::/)'`, every helm provisioning test (the
  mount checks, uninstall inspection, planning, refusals and final removal among them): all 344 passed, run
  `fe034ef7-ec6b-473d-953a-82078ad2a8b3`. Three of those passes skipped their real work because no `farhelm` binary had
  been built in this checkout yet. After `cargo build -p farhelm`, those three ran again in full and passed: uninstall
  stopping a live terminal host (run `87c66105-4799-4e5b-9619-9148ba2c32a3`), and installing then updating Farhelm both
  without SSH and over SSH to this machine (run `b06f8524-8728-449c-b926-ee2d86545005`).
- An extra check outside the project's inventory and CI: `cargo doc -p farhelm-helm --no-deps --document-private-items`,
  with warnings denied, to see that the comment fix's two new links resolve. They do. The command still fails, on about
  170 broken or private documentation links that are already on main, none of them in a file this plan or the landing
  touched.

Reused: nothing from the executor's runs.

Skipped: browser and desktop tests (no page or desktop code changed), a run on a real Mac (the plan settled on
shell-fixture tests instead), the rest of the workspace's Rust tests (the change is one shell function inside the helm,
covered by the tests above), the docs website build (one sentence of prose with no links or components), and hosted CI
(manual-only, not dispatched).
