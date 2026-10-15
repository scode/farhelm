---
kind: changed
---
New OMP sessions default to Guarded, and switching agents in the session launcher now warns when the permission mode
becomes more dangerous, with Undo. Farhelm does not detect OMP approval prompts, so an OMP session waiting for approval
still shows as running or idle, with no waiting alert.
