---
kind: fixed
---
A Claude Code or Codex session you have not touched no longer jumps up the session list, turns unread, and shows a
fresh "last active" time when the agent redraws something on its own while idle, such as Claude's `/clear` suggestion,
Codex's recap of the conversation, or Codex's usage counter. Farhelm now recognizes what these two agents show while
working, waiting for you, and at rest, instead of treating any change on screen as activity.

A Claude Code or Codex session waiting for your answer (a permission prompt or a question) is now shown as waiting
reliably, and moves up the session list when it starts waiting. Its "last active" time is when the question appeared, so
a question left unanswered for hours shows its real age.

Other agents keep the previous behavior, where any change on screen counts as activity.
