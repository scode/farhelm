# Report: Codex forks and app-server conversations can now be restarted

## What this was about

Farhelm offers Restart on a Codex session once it has captured which conversation the session's Codex is in, so that
Restart can resume it. Two kinds of Codex conversation were never captured, so their sessions never offered Restart:

- **Forks.** When a Farhelm session resumes a Codex conversation that another app already has open (Codex in another
  terminal, say), Codex shows a read-only screen, "This conversation is open in another app", whose only ways forward
  are retry and fork. A real session hit this: the user forked and kept working, and Restart never appeared. Codex
  reported the fork to Farhelm's hook with a source of `fork`, which Farhelm did not recognize and refused. Forking
  inside a session was refused the same way.
- **Conversations started through a Codex app-server.** Codex records in each conversation's file who started it.
  Conversations started through Codex's shared background service (which a terminal Codex attaches to) are marked
  `vscode`, and they keep that mark forever. Farhelm accepted only the plain terminal's and `codex exec`'s marks, so
  such a conversation could never be restarted in Farhelm, even after nothing else had it open.

The TODO entry asked for both to be accepted, the fork first and the app-server conversations as their own change,
because the second widens which conversations Farhelm captures. When planning, you settled that the app-server mark is
accepted whatever program wrote it (the conversation file also names that program, its "originator"), and that the marks
for an MCP server and an unknown origin stay refused.

What is on main now:

- **Forks.** A fork, from the lock screen or within a session, now becomes the session's conversation, and Restart
  resumes the fork. It passes the same checks as any other conversation: it must come from the session's own Codex
  process, and its conversation file must exist and name it. A fork whose file is missing is still refused, and the
  session keeps whatever it had captured before.
- **App-server conversations.** A conversation marked `vscode` is now accepted, both when Codex reports it and when
  Restart checks it again at the click. The marks for an MCP server and an unknown origin, and the ones Codex puts on
  its internal helper conversations and subagents, are still refused.
- The implementation notes and the docs page on agent hooks name the newly accepted values, each change has a changelog
  entry, and the TODO entry is removed.

## Things you should know

- **Farhelm now captures conversations started through the shared background service.** Conversations started in a plain
  terminal Codex outside Farhelm were already accepted; what is new is that a conversation someone started through the
  background service, and later resumed in a Farhelm session, can now be restarted there. This is the widening you asked
  to keep separate, and it is its own PR (#5128). Farhelm does not check whether another app still has such a
  conversation open: Restart resumes it, and if another app does have it, Codex itself shows its lock screen, as it does
  when you resume that conversation from the launcher today.
- **Which apps this covers is partly unverified.** The investigation found that the app-server marks its conversations
  `vscode` by default, and the real incident showed that for a terminal Codex on the shared background service. Whether
  the ChatGPT desktop app's own server and the VS Code extension use the same mark was not checked; they probably do.
  The changelog entry says so for the release curation.
- **The fork fix does not get past the lock screen.** A Farhelm session that resumes a conversation another app has open
  still lands on Codex's lock screen. What changed is that forking from there now gives a session Restart can resume.
- **The plan said no release note was needed,** but the repository's rules call for a changelog entry for each fix, and
  the plan's own acceptance criteria asked for one per PR, so both PRs have one.
- No other part of Farhelm reads the mark that says who started a conversation, so nothing else changes behavior.

## Open questions and possible follow-ups

- **Whether the ChatGPT desktop app and the VS Code extension use the same mark.** If either marks its conversations
  differently, those conversations simply stay as they were (not restartable in Farhelm); nothing breaks. Checking needs
  a Mac with the desktop app and a conversation started there. I recommend leaving it until someone reports such a
  conversation without Restart, then adding its mark the same way.

## PRs

- #5127 fix: accept Codex fork as a foreground transition (forks can be captured; docs page; changelog entry).
- #5128 fix: accept Codex app-server conversations as root records (the `vscode` mark; implementation notes; changelog
  entry; TODO removal).

## Checks

Run now, on the stack as it lands (main has gained only plans-queue commits since the stack was based):

- The supervisor's Codex tests and the checks a hook's report passes before anything is read, through the test recorder
  as `cargo nextest run -p farhelm-supervisor --lib -E 'test(/codex/) | test(/^service::report_files::/)'`, run
  `9f7a84a1`: 49 passed. They include the new tests: the accepted list of Codex start sources at the hook's first check,
  including `fork`; a fork captured as a session's first conversation and replacing an earlier one; a fork with no
  conversation file, and an unknown source, each refused for its own reason with the session's capture, its version and
  the process it trusts unchanged; every combination of the accepted and refused marks with three kinds of originator;
  and a `vscode` conversation admitted, offering Restart, and passing Restart's check at the click, while an MCP-marked
  one is refused.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test sleep check,
  `dprint check` on the implementation notes, TODO.md and the docs page, and the changelog format check: all clean.

Earlier runs, superseded: `4171cc02` (#5127 before its review fixes, 47 passed), `b4997a6f` (#5128 before review, 49
passed) and `b3dd5932` (the stack after #5127's review fixes, 49 passed).

Reused: none.

Skipped:

- The end-to-end Codex identity tests: they cover how the hook is invoked, how it finds the session's Codex process, and
  how Codex is launched, none of which changed. The tests above drive the two checks that did change, with real
  conversation files and real process identities.
- The workspace-wide Rust run: nothing outside the supervisor's Codex checks changed.
- The docs website build: the docs change is a phrase in an existing sentence.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort. Neither found a blocker.

- #5127: approved with three small points, all fixed: the refusal tests now check which refusal fired, a test was
  renamed to say what it covers, and the changelog wording was narrowed.
- #5128: one should-fix, fixed: the code comment, implementation notes and changelog entry said every app-server
  conversation is marked `vscode` and named the desktop app as covered, which the investigation does not establish; they
  now say "by default" and mark the desktop app as unchecked. The five small points were fixed too (a test helper's
  comment, a test made robust against reordering, the MCP refusal's reason checked, a test description corrected, and
  the changelog's account of what was already resumable).

The fixes were not reviewed again; run `9f7a84a1` ran after them.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous round's two terminal features (copying a file's
contents with Shift-Cmd-click, and pasting the clipboard's text as an uploaded file), which do not touch how Farhelm
captures a Codex conversation. In the same round, just before this plan, agent-host-tailscale-parity landed (agents can
ask for a Tailscale host when adding a host), and just after it cli-effort-reject-flag-values (`--model` and `--effort`
no longer take the next option as their value). Neither touches conversation capture or Restart either.

An independent reviewer and I read the change, looking at whether accepting the new marks lets Farhelm capture
conversations it should not. It does not: Codex's helper conversations and subagents carry a different kind of mark and
are still refused, as are the marks for conversations started by an MCP server or of unknown origin; a reported
conversation must still come from the session's own Codex process; a fork whose conversation file is missing is still
refused; and nothing else in Farhelm reads the mark. The changes stay inside Farhelm's Codex-specific code, as the
repository's rules for agent-specific behavior require.

#### Fixes made while landing

- **A TODO.md conflict.** The round's three plans each removed their own TODO entry, next to each other, so stacking
  them conflicted there. I removed all three entries; no code changed.

#### What the landing made untrue in the report

Nothing I found. The landing's checks were wider than the report's: the whole Rust test suite ran, not only the
supervisor's Codex tests, including the end-to-end Codex tests the report skipped.

#### Checks

Run once, on the round's three plans stacked on main as it stood when the landing started (agent-host-tailscale-parity,
then codex-fork-and-vscode-resume, then cli-effort-reject-flag-values), so each plan's code was tested together with the
others' and with the previous round's terminal changes. Because agent-host-tailscale-parity raises the protocol version
and the three plans between them touch the helm, the supervisor, the command line and the desktop app, the whole Rust
test suite ran. The SSH test worker was still unreachable, so the slow suites ran on boat sandboxes (x86_64 Ubuntu 24.04
VMs), and clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent` (the build with the desktop app's
  test-automation hooks), all with `-D warnings`: clean.
- On three boats, every test of the workspace and the desktop app (`cargo nextest run --workspace`, built once, split in
  three and run through the recorder), recorded runs 26acb5ca-a85a-4103-b668-59565a8874ce,
  5c9b7564-ebae-4866-bfbb-645723ef7e96 and bd019450-b9de-45cd-a860-5fcf904aa72b: 9,129 passed, one failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. The one failure is the test-trace macro's
  compile check, which builds a scratch crate offline from the machine's own download cache. A boat has no such cache,
  so that test always fails there, as `.agents/boat.md` records, and none of the three plans touches it. Run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  0029d888-b065-4155-8e84-58a9fd7430b7, it passed.
- On a fourth boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  a8513ea8-59fd-45a2-84a9-1c23134391b3: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. One of its steps has an agent's `farhelm agent host add` card (without `--tailscale`) open the Add host dialog,
  whose Add answers the agent's request; the new option itself is covered by the app's and the helm's own tests.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  b7d1edee-04d1-4f1b-8bfc-c51b72da2fe2, the test of the screenshot tooling's list of screens, recorded run
  619f638a-151e-45d7-a959-f641e4d1506d, and the workspace's documentation examples
  (`cargo test --doc --workspace --exclude farhelm-desktop`), recorded run 8ec704c8-a5e7-42a4-9737-2112dfb9f14b: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown, and the docs website build
  (`bun run build`, which checks every link): passed.

Reused: nothing. Skipped: the opt-in real-agent and real-Podman tests, which no plan touches, and any run on a Mac,
since none was available to the landing.
