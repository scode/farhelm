## What this was about

Update, setup recovery and uninstall on a remote Mac could fail repeatedly when the SSH command PATH selected Homebrew's
GNU `stat`. Farhelm passed it BSD arguments. Upload-size reports also stopped working, so an upload's stall timeout
could not arm.

The maintainer chose to support this configuration by always using the Mac's system `/usr/bin/stat`. The change applies
that choice to artifact inspection, upload reports and uninstall, documents it, and archives the resolved triage item.

## Things you should know

Linux retains its existing GNU `stat` selection through PATH. No new production setting or fixed PATH was introduced.
The existing unreleased remote-Mac changelog entry now mentions GNU-first PATH support; no duplicate entry was added.
The code, tests and specification change totals 91 added and removed lines, within the plan's size gate.

Tests run on Linux with owned shell and SSH stand-ins. They substitute only the absolute BSD executable while leaving
GNU `stat` first on the child's PATH. This proves command selection and expected arguments, not native BSD execution.
The metadata regression retains symlink and unchanged-file checks; upload reporting must report the received bytes;
uninstall retains its missing-path, denied-lookup and unsupported-command refusals.

## Open questions and possible follow-ups

None identified within the recorded scope. Other BSD-only commands and real-Mac verification were outside this plan.

## The PRs

- [#3298 — use the system stat on remote Macs](https://github.com/scode/farhelm/pull/3298/changes), draft, based on
  main.

## Checks run, reused and skipped

- Recorder run `cd4649bb-70b7-49e1-a233-6ebf99baae43` passed all 16 selected tests with
  `cargo nextest run -p farhelm-helm --lib -E 'test(remote_metadata_) | test(remote_upload_script_) | test(darwin_upload_ignores_path_stat) | test(/^provisioning::macos::tests::/)'`.
  The other 1,662 cases were selected out. Controlled nextest 0.9.148 used four slots, zero retries and verified pinned
  tmux. Source identity, report and output EOF were complete; no omitted or truncated output, dropped console bytes,
  forced cleanup or runtime `SKIPPED` marker occurred. No failed runtime run preceded this pass.
- That run covered the implementation working tree above `593de1e8`, fingerprint
  `7b7cb3a1509c4002f495abe4456ef048b213dbf4fc22a108f449fbd270d822f2`. Its results apply to the final PR because later
  edits only graduate the outcome and add its PR link. The upstream changes inspected before publication only moved
  queue state and added another plan's report; no executable or contract interaction required a rebase or rerun.
- Passed `cargo fmt --all -- --check`, dprint on changed Markdown, `python3 releasing/check-changelog.py format`, and
  the isolated interpreter's `python -B scripts/check-test-sleeps.py` (429 delays, zero missing rationales).
  `python3 scripts/check-agents-md.py` reported zero errors and one existing archived heading-reference warning.
- Passed `cargo clippy -p farhelm-helm --all-targets -- -D warnings`.
- Skipped full-workspace, browser, desktop, cloud and real-Mac suites: this change selects native commands in the
  provisioning scripts, covered by the focused shell and backend tests. No live installation or remote resource was
  touched.

## Review gate outcome

The required single fresh-context GPT-6.1 Sol reviewer at high effort passed the implementation, tests and specification
with no findings, then checked the final outcome graduation and queue removal with no additional findings. A separate
blind reader confirmed the commit explanation is clear and matches the change.
