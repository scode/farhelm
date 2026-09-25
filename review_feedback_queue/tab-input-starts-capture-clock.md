# Typing in a tab starts the agent's conversation-capture clock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Restart may not offer to resume the agent's conversation, or may resume the wrong one.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F10 / COR-TAB-INPUT-CAPTURE`, tagged **possible**. Anchors and title: `service/connection.rs:487-495`,
`agent_kind/capture.rs:86` — Typing into a terminal tab starts the agent's conversation-capture clock

For restart to resume an agent's conversation, Farhelm has to work out which on-disk conversation record belongs to the
session, for example Claude Code's file under `~/.claude/projects/`. This is called conversation capture. For agents
whose hooks do not report the conversation directly, a scan finds it by timing: the record must appear between 5 s
before and 60 s after the first input byte that tmux confirmed it delivered to the session (`CAPTURE_WINDOW_AFTER`,
agent_kind/capture.rs:86). That anchor is written once per session by `note_first_input`. Its documentation describes it
as the moment input reached the session's pane, "the last moment before which the record cannot yet exist", meaning the
agent's pane.

The input path in the connection handler (connection.rs:487-495) calls `note_first_input` whenever any bytes are
delivered, on any of the session's terminals. It never checks `route.key.terminal`, so keystrokes in a tab count too.
Suppose a user opens a tab, types `git status`, and only prompts the agent a few minutes later. The anchor is then the
tab keystroke, and the agent's record appears after the 60 s window. The scan never claims it, and the session ends up
`UncapturedFinal`, so restart cannot offer to resume. Two worse variants follow. If the user runs the same agent CLI in
the tab within the window, the tab's conversation can be committed as the session's own, which contradicts the docs'
claim that an early anchor only ever causes a missed capture, "never a wrong one". And the tab-anchored window can
overlap another session started in the same directory, so both are marked `Ambiguous`. The documented accepted
limitation covers only one kind of early anchor: automatic terminal replies inside the agent's own pane.

The impact is real only where the scan decides. A hook report outranks every scan verdict, so sessions whose agent hooks
are enabled and do report are unaffected. The suggested fix is to call `note_first_input` only when
`route.key.terminal == TerminalId::Agent`, with a test that input delivered to a tab leaves the session's first-input
time unset.
