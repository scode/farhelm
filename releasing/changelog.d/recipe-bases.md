---
kind: changed
---
Tensorlake recipes now choose a base — Tensorlake Ubuntu, your image or your snapshot — that Farhelm builds on with optional Ubuntu packages, Claude Code and Codex. Recipes that previously reused an image or snapshot need one snapshot build before adding hosts; hosts already created from them are unchanged.

A custom base must boot systemd, allow passwordless sudo, be Ubuntu or Debian with apt, and include bash, curl and git;
the build stops at the first one missing, so an image that served hosts before can now be refused for new ones.
Building bills a temporary build sandbox, and Farhelm installs its own Claude Code and Codex even when the base already
has them.
