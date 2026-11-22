# Grok setup consent omits that the old manual file is replaced in full

Reviewed commit: d04f63f48a2b4a8eecf7c5071b339198a1483896

## TLDR

Enabling Grok hook setup can replace local edits in the file the previous Farhelm guide asked users to create. The setup
prompt leaves that consequence unstated.

## Details

**highest.** Source inspection.

d04f63f48a2b4a8eecf7c5071b339198a1483896, website/src/content/docs/docs/agents/grok.md:16-19, 49-50;
1a53d9d35889d7dde96e31320cc1681dd013737d, SPEC.md:3086-3096, 3979-3986; SPEC_impl.md:4503-4511;
crates/gui/farhelm-gui/src/hook_setup.rs:98-110; crates/farhelm-supervisor/src/agent_kind/persistent_hooks.rs:26-56;
website/src/content/docs/docs/agents/grok.md:16-24

Users upgrading a customized manual Grok setup are told that automatic setup changes configuration permanently, but the
offer and guide omit that it replaces the entire file the prior guide asked them to create. The stated design
intentionally overwrites that path without migration. The product spec covers the named file and requires wording about
the persistent vendor configuration change. Current copy meets that explicit minimum and the implementation spec permits
whole-file replacement. Thus do not report the overwrite itself as a defect or require migration. The narrower
independently actionable issue is a missing consequence in consent and migration help: distinguish updating Farhelm's
own generated contents from replacing a user-authored file and explicitly say edits in that file will be lost. The old
guide shows a user chose the command and expected the file or its three entries to remain theirs to remove. The current
copy describes a change, and the guide says installs or updates, neither of which identifies this exact replacement
consequence for an upgrading reader. The reviewer suggested other for the established wording gap under an expressly
approved design. The conditional consequence is loss of meaningful user-owned local edits, but no actual customized user
file is established. Confidence: wording and overwrite behavior confirmed by source. The ruling on expected refresh of
generated-only contents remains under hook-generated-file-refresh.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`hook-contract/hook-manual-file-ownership`, with `hook-persistence/hp-manual-hook-collision`.
