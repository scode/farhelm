# Sweep-base tests miss state written before refusal

Reviewed commit: 06af3b27a0adf6c357a9def88dbf6bced5fee027

## TLDR

The sweep-base tests can pass even if an unsupported revision creates run state before deflake refuses it.

## Details

`deflake/test-deflake.py:91` exercises the revision helper directly. `deflake/bin/deflake:1375` owns the boundary
between refusal and creating the run directory and current-run pointer. The helper assertions would survive a call-order
regression in the start command.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `deflake-history/f4`.
