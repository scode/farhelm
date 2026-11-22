# Automatic hook setup adds an uninstall cleanup obligation the guide omits

Reviewed commit: 47140bd63486498b7af024b05fa8ade7f8ea849e

## TLDR

Uninstall can leave Grok and Antigravity callbacks pointing to the removed Farhelm executable, and the uninstall
guidance omits cleanup.

## Details

**other.** Source inspection.

470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e, website/src/content/docs/docs/agents/grok.md:16-27, 49-51 and
website/src/content/docs/docs/agents/antigravity.md:29-45; 47140bd63486498b7af024b05fa8ade7f8ea849e,
crates/farhelm-supervisor/src/agent_kind/grok.rs:23-49, antigravity.rs:8-50 and persistent_hooks.rs:60-86;
4603ee87fbf5d66aee13aaa16ba0854012432e6d, SPEC.md:3086-3090 and SPEC_impl.md:4503-4511;
1a53d9d35889d7dde96e31320cc1681dd013737d, SPEC.md:231-240, 884-915,
crates/farhelm-supervisor/src/service/core.rs:6052-6083, 14758-14779,
website/src/content/docs/docs/get-started/uninstall.md:19-34, 56-69

After automatic setup, uninstalling the same Farhelm installation leaves vendor callbacks pointing at the executable it
removed, and the uninstall guide does not direct users to hook removal. The prior manual recipes already used an
absolute Farhelm path and could produce the same stale callback. The new integration introduces an automatic,
Farhelm-managed route to that existing consequence for users who opt in and then launch without creating a file
themselves. It also creates a separate Antigravity plugin location. The uninstall behavior and its deliberate
preservation of vendor configuration are unchanged. The changed product spec correctly says the files survive disabling
setup and the hook guide gives manual cleanup links. The independently actionable gap is surfacing that cleanup during
uninstall, with no behavior change to the uninstaller. The reviewer suggested other for the documentation omission.
Source proves the reference is stale when the installation that wrote it is removed. Whether the vendor shows an error
or disrupts the session is still unverified after investigation. Farhelm source does not implement the vendor's
command-failure policy, and the promise of an empty Antigravity reply covers an executable that actually starts.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`hook-contract/hook-uninstall-stale-callback`, with `hook-persistence/hp-persistent-hook-after-uninstall`.
