# Git hooks can interrupt sweep-base fixture setup

Reviewed commit: 06af3b27a0adf6c357a9def88dbf6bced5fee027

## TLDR

The deflake sweep-base tests can fail before evaluating the base when a developer's configured Git hook refuses a
fixture commit.

## Details

`deflake/test-deflake.py:73` forces the fixture identity and turns off signing. `commit` invokes ordinary `git commit`,
so it still uses the configured hooks path. A fixture history should be independent of those hooks so preflight tests
can reach their assertions.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `deflake-history/f3`.
