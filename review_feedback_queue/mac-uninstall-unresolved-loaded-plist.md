# Missing loaded plist skips uninstall's service-path comparison

Reviewed commit: 25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652

## TLDR

Uninstall can proceed on a connected Mac without establishing that its loaded launchd job came from the planned service
file. The concrete sequence needs an unedited generated plist loaded from another location and then removed. Whether the
best-effort allowance for nonstandard installations accepts this guard omission remains uncertain.

## Details

`crates/farhelm-helm/src/provisioning/macos.rs:232` preserves launchd's absolute loaded path even after the file is
removed. Native `readlink -f` emits no resolved path for that missing file: Apple's
[readlink implementation](https://github.com/apple-oss-distributions/file_cmds/blob/main/stat/stat.c) selects `%R` at
lines 226–267, but the `lstat` failure at lines 363–392 prevents output. `backend.rs:3074` masks that failure through
`tr`; parsing at line 3477 represents the empty resolution as `None`.

`service.rs:1794` compares loaded and planned service paths only when the loaded fragment has a canonical value. Its
later resolved-path checks at line 1812 cover state, binary and library paths, not the loaded fragment. The connected
path can therefore reach `plan.rs:1073`, producing the fixed-label stop and possible library removal. The offline
loaded-job refusal at `service.rs:1769` does not protect this connected route.

`SPEC_impl.md`, "Provisioning", at lines 7577–7579 and 7587–7589 requires matching loaded/planned paths, including when
no file is left; lines 7593–7596 require refusing unresolvable inspected paths. `SPEC.md`, "Supported host setup", at
lines 4818–4823 makes nonstandard setups best effort. No ordinary panel action loading a different path was found:
`plan.rs:670` has no production overrides and lines 697–716 choose the inspected planned path. The different-path
sequence is therefore nonstandard, but it need not hand-edit the plist, and the express ownership guard does not clearly
waive its own rejection cases. Unclear coverage is retained for triage.

Require enough loaded-path evidence to establish the match, or explicitly settle the contract exception. Preserve safe
retry after Farhelm intentionally removes the planned plist but its following stop fails; merely requiring an existing
file would reject that retry too.

Priority: `other`. Confidence: confirmed source guard omission and native missing-file behavior for the stated
nonstandard sequence; supported-product applicability remains uncertain. No runtime occurrence, unrelated-job kill,
security effect or loss of data/processes is established. Proposed drop: none. Possible cover: the best-effort text
above, not a clear accepted exception. No exact planned work, known bug, queue item, ledger decision or filter match was
found. This does not revive the source-refuted claim that the Mac plan bypasses shared containment checks.

Provenance: trailing review `9e3ab60b86025ca74948ededd09b99cd81a2477c..25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`; original `mac-install/mac-install-L3`, corroborated by `mac-planning-L5`,
`MC-I3` and the implementation-contract interface. Implementation introduced in
`c3bee0e586b954f24a12e665a961eb9e96bf50ca`; the reviewed contract is at the full tip above.
