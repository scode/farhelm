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
