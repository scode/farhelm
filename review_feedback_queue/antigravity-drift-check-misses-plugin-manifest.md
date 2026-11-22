# Installer tests assert the Antigravity manifest, while drift preflight only checks command text

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The Antigravity drift check can accept a callback file whose plugin manifest is missing.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC_impl.md:4503–4506; docs/harness-drift-tests.md:27–40;
crates/farhelm/tests/e2e/real_agent_capture/drift.rs:190–236,525–579,603–618,641–664,686–725;
crates/farhelm-supervisor/src/agent_kind/persistent_hooks.rs:123–178

For the documented automatic plugin route, the manifest premise is precisely
~/.gemini/config/plugins/farhelm/plugin.json beside the plugin's hooks.json. A working separate
~/.gemini/config/hooks.json can report independently and needs no plugin manifest. The preflight uses its own HOME and
reads either callback JSON file. It recursively shell-splits command strings and accepts an executable basename of
farhelm with an argv prefix internal hook --vendor antigravity. It does not read plugin.json, verify event
placement/executability, or observe vendor discovery. In the absent sibling manifest case, with no alternative reporting
hook, a run can advance past preflight and end up with Failed for missing capture, rather than CouldNotRun for unmet
installation. Source predicate confirmed; actual discovery/failure timing beyond the documented configuration contract
was not observed. The existing installer test asserts exact managed manifest and callbacks and the untouched manual
file. Search found no targeted drift preflight check for a callback file without its sibling manifest. Launcher code
does not change this read-only check. The reviewer suggested other. ; identified.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-supervisor-checks/HSC-02`.
