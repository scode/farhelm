# TODO.md: the board, tldr todo and the todo flow

The root `AGENTS.md` section "TODO.md", moved here unchanged, including "tldr todo" and the todo flow. Read it when
asked what TODO.md holds ("tldr todo", "what's in the todo" and similar) and when a message starts with "todo flow,".
The rules every task needs (no entries on your own initiative, removal with the fix, freshness, the todo flow's merge
condition) are also in `AGENTS.md`, "TODO.md and FLAKES.md", word for word.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## TODO.md

`TODO.md` is the maintainer's running list of wanted fixes and features. When a PR addresses an entry, remove that entry
in the same PR — the file only ever describes what is still wanted. Do not add entries on your own initiative; they are
the maintainer's. The exception is a deflake run (`deflake/AGENTS.md`), which records the flakes it finds as Deflake
entries. When a Deflake entry is removed, remove the matching line in `deflake/known-flakes.txt` in the same PR: the
deflake sweep reads that file as its exclusion list, so a stale line is a test that never runs again.

"tldr todo", "what's in the todo", and similar requests mean the FULL list, grouped under the file's own bucket
headings, in the file's order, one to two sentences per entry: what it is and, when the entry says so, why or the first
step. Every entry, not a selection — the point is to see the whole board at a glance. Bold a short handle at the start
of each line so an entry can be referred to by name afterwards.

### Todo flow

A message starting with "todo flow," makes the session a todo-capturing session for the rest of the session. The point
is that captured entries land on main without the maintainer having to remember to ask: a TODO edit left sitting in a
working copy is easy to forget, and a forgotten one is lost the next time the workspace is reused.

In such a session, every TODO.md change carries an implied "jjstack make a PR and merge it" once discussion of the entry
is done, meaning the entry is written and nothing about it is waiting on the maintainer's answer. Load the `jjstack`
skill, put the change on the latest main, open the draft PR, and land it per that skill. A later revision of an entry
that already landed is a new PR of its own. The only validation is `dprint check TODO.md`. These commits and PRs skip
the `scode-commit-msg-reviewer` cold read, even where standing instructions say to always use it: for a one-line TODO
capture it is not worth the wait.

Before adding or revising an entry, and before answering any question about what TODO.md currently holds (a "tldr todo"
included), fetch and rebase the working copy onto the latest main per jjstack: `jj git fetch`, then rebase the
working-copy change onto `main@origin`. Other sessions land TODO.md changes throughout the day, so a checkout that was
current when the session started goes stale within it, and an answer read from it can miss entries or list ones that are
already gone. In a stack (a deflake run, plan or triage execution), fetch and read main's `TODO.md` instead of rebasing
mid-run, and keep both sides when the stack's own rebase meets a `TODO.md` conflict.

NOTE: The implied merge applies ONLY while TODO.md is the only file the session has changed. If anything else has been
touched — code, specs, other documentation, even an unrelated edit still in the working copy — do not commit or merge
anything automatically. Say so and ask how to proceed. The implied merge is for pure TODO capture; it is not a way to
land other work without review.
