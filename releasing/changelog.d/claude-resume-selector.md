---
kind: fixed
---

A Claude session launched with a command that already picks a conversation (`--continue`, `-c`, `--resume`, `-r`,
`--session-id`, `--from-pr`, `--teleport` or `--fork-session`) or contains a bare `--` could resume the wrong
conversation, or start a fresh one: Farhelm built its Resume by adding `--resume` to that command. Creating such a
session now fails up front with a message saying so, unless it comes from a profile that sets its own resume command.
Any argument spelled exactly like one of those counts, including a wrapper's own arguments, so a wrapper such as
`sh -c '…' w {cwd} claude` declared as Claude also needs its own resume command. Sessions created before this change
keep the resume command they were given and can still resume the wrong conversation; recreate them, or start them from a
profile that sets its own resume command. Farhelm's built-in Claude launches are unaffected.
