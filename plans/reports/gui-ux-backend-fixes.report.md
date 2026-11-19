# Backend fixes from the November 18 triage

## What this was about

Six fixes from the November 18 triage cover retired-session port tunnels, shutdown after terminal recovery, agent
endpoint paths, command display, slow shell-tab replies, and approval answers that arrive after expiry.

## Things you should know

A supervisor refuses a port tunnel for a retired session even when the helm's cached entry is unreadable. After a
terminal reconnects, replacing the macOS supervisor checks the recovered terminal. This avoids an unnecessary
three-second wait and the false shutdown warning that went with it.

An endpoint can contain an at-sign in its path. Farhelm still refuses an at-sign in the host part, where it could
introduce credentials. Displayed start and resume commands quote literal special characters. The actual command and its
user-provided wrapper work as before.

Open and Close tab now answer within the host reply bound. A slow tab change can finish while the desktop stays
connected and keeps any open launcher draft. The desktop shows an expiry notice if an approval answer arrives too late.
If Farhelm saved an Always allow setting before discovering the expiry, the setting stays saved.

On November 19 the landing monitor took Tensorlake pause attribution out of the plan. If a host paused by Farhelm
resumes elsewhere and pauses again, Hosts can still say “Suspended from Farhelm” with the first pause's time. The second
trial reached 265 changed lines against an allowance of about 200. Review found that a new Suspend can inherit the first
pause's time if it races with recording that the host ran. Retry after a failed clear can also restore that time.

## Open questions and follow-ups

The six reviewed fixes are ready to land. The Tensorlake outcome remains pending in triage and can be scheduled on its
own.

## Pull requests

- Refuse port tunnels for retired sessions before dialing. [#4234](https://github.com/scode/farhelm/pull/4234/changes)
- Avoid a false shutdown warning after terminal recovery. [#4236](https://github.com/scode/farhelm/pull/4236/changes)
- Accept at-signs in agent endpoint paths. [#4237](https://github.com/scode/farhelm/pull/4237/changes)
- Quote literal words in displayed agent commands. [#4239](https://github.com/scode/farhelm/pull/4239/changes)
- Keep a stalled shell-tab reply from resetting the desktop connection.
  [#4243](https://github.com/scode/farhelm/pull/4243/changes)
- Return an expiry refusal for late approval answers and record the deferred outcome.
  [#4250](https://github.com/scode/farhelm/pull/4250/changes)

## Checks

- Reused recorded Linux nextest runs. The commands below are abbreviated and the recorders retain the exact filters.
  `cargo nextest run -p farhelm --test e2e` passed six tunnel tests at `fc40c9e7`
  (`f3730aa4-863f-4048-a213-d892f8578680`). `cargo nextest run -p farhelm-supervisor --lib` passed seven terminal-hold
  tests at `a17a3180` (`5d9dbcfa-4665-4a3a-9060-20d066bea302`).
- `cargo nextest run -p farhelm-proto --lib` passed three endpoint and command display tests through `843d8911`
  (`98e03bce-3dca-4924-b952-ee99194916a6`). `cargo nextest run -p farhelm-helm --lib` passed 36 channel and approval
  tests through `dc232662` (`9542f74f-db4f-489d-91f6-eff8af758586`). Recorder sources matched the respective drafts in
  the paths exercised. No selected test reported a substrate skip.
- Reused focused Clippy for the helm, supervisor, protocol and end-to-end targets. Comparing each old and new draft
  confirmed unchanged backend code, tests and contracts. The only new change on the stack is the deferred ledger entry.
- Checked formatting with `dprint check`, whitespace with `git diff --check`, and the feedback and archive entries
  against their expected content. `python3 scripts/check-agents-md.py` reported four existing citation warnings and no
  errors. The ledger edit affects only triage bookkeeping, so it needed no runtime tests. The local Linux run used a
  stand-in for the macOS bundle layout in #4236 and skipped native macOS supervisor replacement and hosted CI.

## Review

Each of the six fixes passed its required independent source review. Review changed the tunnel test to observe refusal
before a dial, kept in-flight tab requests cancellable before their reply deadline, and made the expiry test exercise a
verdict the waiter actually received. Fresh readers checked the final triage record and delivery report.

### Landing

#### What landed with it

It landed in a round of two plans, after gui-ux-desktop-fixes. Since its base, main gained exited-session-no-terminal,
which reports a session whose terminal is gone so the desktop app shows a recovery card. This plan changes nothing in
what the helm, the supervisors and the desktop app send each other. Its fix to the wait when the Mac app replaces its
supervisor after a terminal reconnected holds with that plan's Restart changes: for a session restarted since, or one
whose terminal is gone, the replacement behaves as it did before this fix, so at worst that session brings back the
three-second wait and the warning. The independent reviewer found no other interaction. The two plans' triage ledger
entries were kept side by side; each executed outcome now appears once in the archive, and the Tensorlake pause
attribution outcome stays pending in the ledger with its feedback file, as the answer to this plan's block asked.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The six fixes are small and
match their outcomes, with one departure: for slow shell-tab replies, the outcome named the existing way the helm bounds
how long it waits for a host's reply, and the fix instead adds a second, general one so that the helm could abandon the
request part way. Abandoning it does not stop the tab from opening on the host, so that is extra code for a small gain.
The behavior is correct and tested, and I left it; folding it into the existing mechanism is a possible follow-up.

#### Fixes made while landing

- **Displayed commands quoted Farhelm's own placeholders.** The fix that quotes literal braces, `!` and `^` in a
  displayed command also quoted the placeholders Farhelm fills in at launch, so a Codex session's command read
  `-c '{codex:trusted-cwd}'`, as if the user had typed that word. The workspace test run caught it, through the helm's
  session-list contract test (an example session list the helm must keep producing). A placeholder now stays bare, as
  written, and the display test covers one.
- **A ledger note was reworded.** The note on the deferred Tensorlake outcome ended "a Retry after clearing the old
  timestamp fails", which reads as if Retry fails; it now says a Retry after an attempt to clear the old timestamp
  failed.

#### Things you should know that the report does not say

- When an approval expires, the agent is told nobody answered and its request is refused. An answer that arrives after
  that, Allow or Deny, now gets a notice that it did not take effect, where before it looked accepted; adding a host
  already worked this way. For a late Deny the outcome is the refusal the user wanted anyway, so the notice is accurate
  if slightly surprising. Possible follow-up: say that the request was already refused.

#### Checks

The two plans change the desktop app's GUI crate (with reference screenshots), the helm, the supervisor, the protocol
crate and one of the command line's end-to-end tests, so the landing ran the whole Rust workspace suite, the desktop
app's own tests, the app's end-to-end smoke and the reference screenshots (goldens). Builds ran locally; the workspace
suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main with the triage ledger's entries kept side by side:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy --locked -p farhelm-gui --all-targets --features agent -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats
  (`5342216e-bb7d-42fd-9b91-ca4e678c1f5f`, `ca1387dd-299b-4699-b411-c79e800d0a81`,
  `4c95deaa-83c0-4d00-9b38-8c3b8914fd93`): 8084 of 8087 passed, with no runtime skips. The helm's session-list contract
  test failed because of the placeholder quoting fixed above. The macro compile-contract test failed as it always does
  on a boat, since it resolves crates from the local cargo cache, and passed locally
  (`cargo nextest run -p farhelm-testtrace-macros`, `87c5f9c4-0890-4393-ae5c-9de9ccb0463f`). The test that a scoped Stop
  leaves jobs in a stock tmux server alone failed its own setup check once: the tmux pane's job was still in the
  session's launch scope when the test looked. Nothing in this round touches tmux or launch scopes, and the test passed
  when run again on a boat (`52b91ef2-a4fe-45cd-a7b0-cebe8fb916e8`) and three times locally
  (`5535b0da-c395-498e-a062-aa0ab352c2ff`, `cf0e04f5-5376-41e4-9cd0-1171cc8128e7`,
  `af18a590-134f-478a-a5e5-d5511b49c2ed`), so I treat it as a one-off and have not logged it as a flake.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`b081f5b1-dce7-46b0-af3e-7d7e0622e433`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, and the clipboard
  paste step was skipped, as always on Linux (`05a7cdf6-eb82-4289-bd54-05d1011e5ca6`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`e46d848a-0c97-43e0-be7a-47b091f995f8`).

On the fixed tree (bare placeholders in displayed commands, the dropped-file line, a comment and a ledger note):

- `cargo fmt --all -- --check`, `dprint check` and `cargo clippy --all-targets -- -D warnings`: passed.
- The protocol crate's launch display tests and the helm's HTTP contract tests,
  `cargo nextest run -p farhelm-proto -p farhelm-helm --lib -E 'test(/session_launch::/) | test(/^http_contract_tests::/)'`,
  locally: 10 passed, the session-list contract test among them (`fc861d2e-ae20-4beb-9a23-2369e95761d3`).
- The GUI's terminal attachment and template tests,
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^terminal::attachments::/) | test(/^templates::/)'`, locally: 343
  passed (`f74ba7dc-fbe0-4a14-85eb-b4e078f493ca`).

The rest of the workspace suite, the desktop app's tests, the smoke and the goldens were reused for the fixed tree.
After them, only the displayed-command rule, the dropped-file line, a comment and a ledger note changed; the focused
runs above cover the first two, and no reference screenshot draws the dropped-file line or a command with a placeholder
(the goldens passed on the delivered tree, which still quoted placeholders).

Skipped:

- The Mac-only checks: no code that only builds for the Mac changed. The fix to the wait when the Mac app replaces its
  supervisor is shared code, and its test ran on Linux with a stand-in for the Mac app's layout; nothing ran on a Mac,
  and the desktop changes were not rendered on macOS.
