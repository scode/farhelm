---
kind: added
pr: 844
---

Cursor is a harness choice in the launch composer, using the same generic launch path as any other command. This is
launch only: Farhelm does not track a Cursor session's conversation and cannot resume one, so the session shows no
conversation identity and Resume stays unavailable for it. No Cursor configuration is touched and no hooks are
installed.
