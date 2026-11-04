---
kind: added
---
Settings has a new Tensorlake section, where you can save a Tensorlake API key and test it, and create the SSH key
Farhelm uses for Tensorlake sandboxes, with the one-time `tl` command that registers it with your account (it works from
any machine where `tl` is signed in). Farhelm stores the API key unencrypted in the helm's state and never shows it
again. A switch lets adding a Tensorlake host skip its billing confirmation; it is off by default.

This is the first part of Tensorlake sandboxes as hosts. Adding, suspending and removing Tensorlake hosts arrive in
follow-up changes; if this ships without them, the entry should wait for them or say the section prepares an account.
