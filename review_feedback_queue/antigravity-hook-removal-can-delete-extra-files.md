# Antigravity hook removal advice includes unrelated plugin files

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

Settings directs users to remove Antigravity’s entire plugin directory even though setup manages two files. Any other
files in that directory would also be removed.

## Details

**highest.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/hook_setup.rs:106-114;
crates/farhelm-supervisor/src/agent_kind/antigravity.rs:8-50; 199f513604dd6204f8a107e4314bcf8f9280167e
website/src/content/docs/docs/agents/antigravity.md:34; 1a53d9d35889d7dde96e31320cc1681dd013737d
website/src/content/docs/docs/agents/antigravity.md:29-37

The setup accepts an existing Antigravity plugin directory and manages `plugin.json` and `hooks.json` without checking
the rest of the directory. Settings and the guide tell the user to remove the directory itself. Following those
directions can discard any unrelated files there. The source establishes this conditional route. The review did not
inspect an affected user directory or observe a deletion. Settings also omits the older manual hook entry in the shared
configuration file, which the pinned guide already addresses. Align Settings and the guide with the two managed files,
allow directory removal when empty and keep the separate directions for the manual entry.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-settings/HS-2`.
