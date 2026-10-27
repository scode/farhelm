# Reopening an open host menu loses keyboard navigation

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

Right-clicking the same host avatar again while its menu remains open clears that menu's keyboard navigation. Closing
and reopening it restores navigation.

## Details

Suggested bucket: **other**, for recoverable keyboard-navigation loss. Confidence: **confirmed source lifecycle
defect**; actual native focus presentation was not exercised. No wrong-target operation or user-work loss is claimed.

`crates/farhelm-ui/src/hosts.rs:4133–4136` calls `begin_open` unconditionally. The rail callback retains the same open
host (`2554–2562`), so the existing menu/buttons remain mounted (`4447–4450`). `begin_open` clears item handles and
installs a mount-only focus intent (`4117–4131`), but item handles register only through `onmounted`, such as
`4482–4483`. The reconciliation effect retains/removes handles; it cannot recreate them (`4186–4226`). Arrow, Home and
End navigation then returns for absent handles (`src/menu_panel.rs:889–902`). The outside-dismiss listener explicitly
excludes the expanded avatar (`1124`; avatar markup at `hosts.rs:705,712`), so no narrow timing race is required.

Preserve mounted handles for an already-open invocation or explicitly remount before clearing them. The source mechanism
remains at pinned tip `9b19f8d754bf7658761ef246cfda0ea8397f4f21`; no missing source premise remains for it.

SPEC_impl.md:789–790,831–844 requires initial focus and roving navigation. The queued blur/focus concern and prior
independent-focus decision have different causes. No exact Planned, unlanded-plan, BUGS, queue, ledger or filter
coverage was found. Possible cover: none. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g011/g011-f2`, found during synthesis.
