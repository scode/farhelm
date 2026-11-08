---
kind: fixed
---

Delivering recipe credentials to a Tensorlake host now preserves Claude Code or Codex settings saved while delivery is
in progress. Repeated conflicting saves leave a warning and retry on the next connection.

For curation: a save landing in the instant between Farhelm's final check and its replacement of the file can
still be lost, since there is no lock on the host. An agent that saves its own older copy of the settings after
delivery finished can still undo the delivered values; this change does not cover that direction.
