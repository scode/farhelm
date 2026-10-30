---
kind: fixed
---

Codex sessions now always run without Codex's shared background server, keeping their own environment and processes, and no longer show its "Running without the shared background server" warning. Codex older than 0.156.0 no longer starts under Farhelm, and a custom Codex command that puts `{farhelm_args}` after `exec` now fails.

Legacy sessions started before Farhelm 0.22 from a typed command or profile keep their old behavior until recreated. Custom commands using `--remote`, `codex agents`, or `codex queue` also cannot accept `--no-daemon`.
