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
