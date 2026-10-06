## What this was about

A fresh Farhelm install ended with launch and uninstall advice but no invitation to share feedback. The maintainer
agreed on a five-line message that welcomes even a quick throwaway comment, explains how to send private feedback
through the sidebar's ? menu, and offers a public GitHub issue as an alternative. Updates were to keep their existing
message.

The installer now prints that exact invitation on every successful fresh install, including when tmux is missing or too
old. It follows the launch instruction when that instruction is present, precedes any renamed-file notice and uninstall
advice, and leaves the tmux warning last.

## Things you should know

The agreed wording, line breaks and three-space continuation indentation are preserved. The GitHub URL is cyan in
terminal output and plain when redirected or color is disabled. This is an invitation only: nothing sends feedback
automatically. Update output remains unchanged.

The invitation reads:

```text
💬 I'd love to hear what you think, even a quick throwaway
   comment. In Farhelm, click ? at the top of the sidebar and
   choose Send feedback; it comes privately to me, the maintainer.
   If you'd rather discuss it in the open, file a GitHub issue
   at https://github.com/scode/farhelm/issues instead.
```

Execution required no scope change. The TODO entry was removed and an Added changelog fragment was included. The only
review suggestion was to clarify the installer's comments about message placement; that was addressed.

## Open questions and possible follow-ups

None.

## PRs

- [#1674 — feat: invite feedback after fresh installs](https://github.com/scode/farhelm/pull/1674/changes), draft;
  bookmark `plan/installer-feedback-prompt/01-invite-feedback`, change `krxnsuyxryrwxlumoywwpwvrlzlywknn`, pushed commit
  `c6bf907ee02cf41a9a1ef960b2168837cef557ec`.

## Checks run, reused and skipped

- Run: `sh -n scripts/install.sh` and `shellcheck scripts/install.sh scripts/test-install-sh.sh`, including after the
  final comment clarification; both passed.
- Run: `dprint check TODO.md` and `python3 releasing/check-changelog.py format`; both passed.
- Run: recorded `bash scripts/test-install-sh.sh`, selection “installer feedback closing reports and installer shell
  acceptance,” one fixture at a time, tmux mode none. Run `2c30f85d-eec7-4a8f-91c6-fe2a96f1caff` passed 514 checks with
  zero failures. It covers exact fresh and update messages for adequate, missing and old tmux, the renamed-file
  placement, and terminal/plain output.
- Reused: that installer acceptance pass for the final commit, because the only later edits clarified comments;
  executable behavior and test expectations are identical. No upstream changes needed a rebase before delivery.
- Skipped: Rust, browser and native macOS uninstall suites, because only installer report text and its shell fixtures
  changed. The Linux fixture suite runs the installer with a simulated Mac platform against private fixture homes; it
  does not exercise a native Mac desktop launch.

## Review gate outcome

Independent Opus 5.5 high and gpt-6-astra high static reviews found no correctness, design or acceptance defects. Opus's
optional comment clarification was applied. Both reviewers received the full test-authoring checklist. The commit and PR
title passed a fresh-context wording read; the PR body is empty because the title and diff explain the change. Runtime
evidence comes from the recorded fixture suite, not from the reviewers, who ran no runtime tests.

### Landing

Landed on 2026-10-06 (UTC) as #1674 (the installer invites feedback after a fresh install), one squash commit on main.

#### What else was on main

Between the commit the change was built on and the landing, the home-tab-trailing-slash plan landed (#1673, trailing
slashes removed from the directory a terminal process starts in). It changes the supervisor's terminal start, not the
installer; the only file both touch is TODO.md, where each removes a different entry, and the rebase onto it applied
without conflict. Otherwise main gained only the planning queue's own bookkeeping.

#### Review before merging

A separate reviewer that had not worked on either plan checked both before anything merged and found nothing outside the
PR that the change breaks. The new message is only in the fresh-install branch; the update branch is unchanged, and the
Mac app's automatic updater decides success from the installation record the installer writes, not from its output,
which it only copies into a log. The uninstall acceptance test only looks for specific phrases in the installer's
output, never the whole closing message, and no documentation or website page quotes that message. The message's
directions (the ? button at the top of the sidebar, then Send feedback) match the spec. The website's install page says
the installer "finishes by telling you Farhelm is installed and how to uninstall it later"; that is still true, though
it does not mention the invitation.

#### Checks

- Reused: the report's checks. The rebase brought in only #1673, which changes no file the installer or its test uses,
  so the installer test run still applies to what landed.
- Skipped: running anything again during the landing, for the same reason.

Nothing in the report above was made untrue by the landing.
