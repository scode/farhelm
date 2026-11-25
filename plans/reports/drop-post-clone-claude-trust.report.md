## What this was about

With Claude Code's workspace-trust switch on, starting another agent in a new managed checkout could mark that directory
trusted in Claude Code. Farhelm now makes the edit only while starting Claude Code, after the checkout and its
post-clone command finish. The trust hint above the terminal is available only for Claude Code sessions started as an
agent. This follows the decision to apply file-based trust when the matching agent starts.

## Things you should know

When Farhelm adds trust on the first launch in a new managed checkout, it records the settings file and directory before
the edit. Farhelm uses that record to try to remove the trust entry when it archives the checkout or retires it because
the directory is missing. If Farhelm is recovering the initial launch and the login shell now selects a different Claude
settings file, it keeps the original cleanup record and lets Claude ask about trust in the new file.

The **Claude Code** docs and **Start a session** guide now describe when Farhelm edits the file and when it removes the
entry. The Claude Code Settings page shows the same behavior in both appearances.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4992](https://github.com/scode/farhelm/pull/4992/changes) moves Claude workspace trust to launch and updates
  Settings and the docs.

## Checks

- Pinned `cargo nextest run --locked` selected 72 library tests across the protocol, supervisor, helm and GUI for
  workspace trust, managed checkout requests, agent approvals, and terminal trust hints. The initial candidate based on
  `3c158f6e` passed 71 of 72 in retained run `7b4a223d-b319-4eaf-8b60-56f3d5429640`. The failure was in the newly added
  shim test, which invoked the wrong child fixture. The 71 passes still apply because the production correction affects
  only the launch shim's handling of an existing trust cleanup record. The follow-up
  `cargo nextest run --locked -p farhelm-supervisor --lib` selected exactly the initial managed launch and recovered
  create cases. Both passed on corrected source `fb3ee684` in run `14384d4d-4618-40c4-a0a1-09cc22570e03`, covering file
  edit ordering, cleanup, and retries using the same or a different config.
- `python3 scripts/check-gui-goldens.py --scene settings-claude` showed the expected copy changes against the prior
  references in Linux light and dark in run `7cebc803-ea73-4c01-8eb9-0947443b56bc`. The full `--update` render of 530
  images in run `ce6930c6-a0ce-4f12-a0a1-61413f20fbc0` changed only those two images. Both published references match
  the reviewed captures byte for byte.
- `bun install --frozen-lockfile` and `bun run build` built 41 pages and validated internal links. Rust and Markdown
  formatting, the test-delay audit, changelog checks and whitespace checks passed.
- `cargo clippy --locked -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm-gui --lib --tests -- -D warnings`
  passed.
- The focused tests cover the changed rules, so a full workspace run would add little evidence. A stand-in agent with
  isolated settings exercises the actual write and cleanup without relying on vendor execution. The full screenshot
  capture used the local Linux renderer. The change uses the same text on macOS.

## Review gate

The required fresh-context gpt-6.2-sol high source review identified the fixture wiring, a recovery case that could lose
the original archive record, and stale wording in the specs. All findings were fixed and the reviewer found no remaining
issues in the corrected source. The PR also includes the two screenshot references that were verified separately after
the source review.

### Landing

#### What landed with it

Only this plan landed this round, through its PR and the landing fix
[#4998](https://github.com/scode/farhelm/pull/4998/changes). Since the plan's PR was written, main gained the tab
switching shortcuts, the recipe card and billed size fixes, and docs and TODO entries, none of which touches Claude's
trust. Earlier rounds landed the sandbox workspace trust plan and the hooks and trust triage. Part of their work passed
the post-clone trust choice along when an agent's request waited for approval; this plan removes that, and the landing's
review checked that nothing else depends on it.

The review found the change right when the helm and the host's supervisor are the same build. With Claude's trust switch
on, trust is now applied whenever a Claude Code session started as an agent (not as a custom command) launches in a
folder: New, Clone, Restart with, Replace with and agent requests, as before. Another agent or a custom command never
gets it, and plain Restart and Replace keep the saved launch's choice. Only the launch that creates a new managed
checkout also records the trust so that archiving the checkout removes it. Checkouts whose trust Farhelm recorded before
this change still have it removed on archive, and sessions created before this change still restart and recover
normally. The review counted no blockers, one should-fix (the first item of the last section) and five small points: the
changelog draft's missing scope is folded into the landing fix, and the later-session trust, the docs wording, the
ragged page lines and the weak tests are the other items of the last section.

#### Fixes made while landing

- **The gap with an older supervisor is now stated.** #4998 adds a SPEC_impl.md sentence describing it (below), and a
  paragraph to the changelog draft saying the same and that a session started as a custom command in a new checkout no
  longer gets the trust edit or the trust hint, which the draft left out. It changes no code.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PR rebased onto main as it stood at the start of the landing (`2530c3c1`); the rebase conflicted only in
TODO.md, where both sides removed entries and I kept both, and every other file's change is exactly what the executor
pushed. The rebased commit was `f3c48d5b`. The only later change is the landing fix
[#4998](https://github.com/scode/farhelm/pull/4998/changes), which adds one SPEC_impl.md sentence and a paragraph to the
plan's changelog draft; after it I ran `python3 releasing/check-changelog.py format` and `dprint` on both files, which
passed. That final tree is what main now holds outside the plans queue.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `6872dd6b-bb1a-4099-875e-ca9de945168c`): all 8714 tests passed, with no runtime skip. The plan changed the helm's
  session creation, agent requests and workspace trust, the protocol's checkout request, the supervisor's launch and the
  GUI's trust hint and Settings, with their tests; the whole suite was the narrowest run that covered them and their
  callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `bdde9e24-3965-47da-b1c0-d316b99a7998`): all 184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `52b980aa-eb26-4f22-89f6-ba16374d505b`): 50 steps passed, one skipped by design on Linux (paste).
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `ec8ddfc7-8778-458d-ac00-0287898181dc`): passed, including the Claude Settings page whose reference the plan updated.
- The screenshot batch tool's test, `python3 -B scripts/test-gui-screenshot-batch.py`, locally (recorder run
  `b7205bef-5c30-4be9-92a3-2e47d2ea75ee`): passed.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. Two docs
  pages changed (Claude Code and Start a session).
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: failed only on the unwrapped
  triage archive entries main already has from the review queue triage record (#4978).

Reused: none. The executor's runs were focused selections on an older base, and everything they covered ran again above.

Skipped: a host running an older supervisor, the case the last section describes; a real Claude Code launch, which the
tests replace with stand-ins; a Mac run, since nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing, though the report does not mention the older supervisor case below.

#### Things you should know that the report does not say

- **(The review's should-fix) With a host whose supervisor has not updated yet, archiving a Claude checkout can leave
  its trust behind.** A new checkout request no longer asks the supervisor to trust the folder after cloning. A
  supervisor from before this change takes that as no post-clone trust, but still trusts the folder when a Claude Code
  session started as an agent launches there, without recording it for removal. So until that host's supervisor updates
  (auto-update off, or not run yet), with Claude's trust switch on, a Claude Code session started in a new managed
  checkout on it leaves a trust entry that archiving the checkout does not remove, and a later checkout that gets the
  same folder starts out trusted without Claude asking. The review's suggested fix is for the helm to keep telling older
  supervisors to make the post-clone edit (with its cleanup record) exactly when the launch is Claude Code started as an
  agent with trust; the updated supervisor ignores that request, so only older supervisors would act on it. I did not
  make that change at landing: it adds per-agent logic in shared helm code and changes which checkout requests count as
  a retry of the same one, so it deserves its own PR and review. I recommend it as a prompt follow-up; say if you want a
  TODO entry.
- **Trust added by a later Claude session now outlives the checkout.** If a checkout is first started with another agent
  and Claude Code is started there later (with Restart with or Replace with), the trust that later launch adds is not
  recorded, so archiving the checkout does not remove it. That follows your decision to apply trust only when Claude
  Code itself starts, with removal tied to the checkout's creation, but the report does not say it. No follow-up
  recommended unless you want removal for later launches too.
- **A session started as a custom command no longer gets trust in a new checkout.** Before this plan, the post-clone
  edit trusted a new checkout whatever the session ran, so a custom command that runs Claude Code got trust and the
  hint. Now only a session started as Claude Code itself does. That follows the same decision; no follow-up recommended.
- **The docs narrow who the switch covers.** They now describe the switch as applying to New and to agent requests that
  ask first, leaving out Clone, Restart with and Replace with, and agent requests that run without an approval card. By
  the review's reading of the code, the switch still applies to those, so the wording should be widened back; say if you
  want a TODO entry.
- **Small points.** The Start a session page has ragged line breaks, one line about 150 characters long (the formatter
  does not check `.mdx` pages); and two helm tests now assert little about trust that other tests do not already cover.
  Neither needs a follow-up on its own.
