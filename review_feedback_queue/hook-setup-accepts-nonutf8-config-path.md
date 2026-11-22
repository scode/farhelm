# Persistent hook setup writes to a non-UTF-8 path instead of reporting it as unsupported

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Persistent hook setup can write to a non-UTF-8 configuration path and report success even though the path contract calls
for refusing it.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:5168-5174; SPEC_impl.md:4508-4515;
crates/farhelm-supervisor/src/service/core.rs:6066-6089,6210-6226,14771-14787;
crates/farhelm-supervisor/src/agent_kind/grok.rs:25-35; crates/farhelm-supervisor/src/agent_kind/antigravity.rs:17-30;
crates/farhelm-supervisor/src/agent_kind/persistent_hooks.rs:26-56; website/src/content/docs/docs/agents/grok.md:22-24

SPEC.md says every surface that encounters a non-UTF-8 path refuses it with a clear diagnostic. Supervisor
initialization rejects bad executable and state paths, but reads GROK_HOME into a native PathBuf, checks only that it is
nonempty, and passes it to hook setup. On a supported Unix filesystem that accepts such names, an otherwise healthy,
opted-in start with a non-UTF-8 GROK_HOME and valid state, executable and session paths reaches the named file in its
original bytes and can report Installed. HOME fallback for both installers uses the same unchecked mechanism. This is a
source-verified gap in setup refusal and diagnostics. It does not silently convert a pathname to different text and does
not demonstrate any write outside the operator's configured location. Vendor behavior with a non-UTF-8 config path is
unnecessary to establish the contract gap and was not assessed. The reviewer suggested other. Validate the selected
vendor destination as UTF-8 before opening or modifying it and let the existing optional setup failure path explain the
problem while the agent launch continues.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`hook-persistence/hp-nonutf8-setup-directory`.
