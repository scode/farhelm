# Duplicate uninstall advice points to a removed owner

Reviewed commit: 76e08553fe1b166d38d5c90d434f52a30a3dba3d

## TLDR

After removing a duplicate machine's owner entry, Uninstall on the remaining entry tells the user to uninstall through
the entry that no longer exists. The remaining entry needs Retry first.

## Details

Suggested bucket: **other**, for wrong recovery advice. Confidence: **confirmed source mechanism**; runtime frequency
was not measured. Retry and Remove remain available, and no security or work-loss consequence is established.

`crates/farhelm-helm/src/provisioning/service.rs:1319–1340` supplies no twin when the owner is absent.
`provisioning/uninstall_refusal.rs:83–90` turns that absence into “another entry (host N)” but still prescribes choosing
Uninstall there. Lines `95–97` mention Retry only after changing the remaining entry to point elsewhere.

This is an ordinary retained state after owner removal. `manager.rs:3301–3317` freezes duplicates until a nudge; removal
stops only the selected actor (`2752–2778`), and registry reconciliation does not nudge the unchanged remaining
duplicate. The existing regression at `8030–8105` explicitly verifies removal, continued frozen state and recovery
through Retry. Tailor the absent-twin remedy to that path.

Possible cover: reviewed `SPEC.md:211–214` requires Retry after removing the owner and `164–165` requires a useful
uninstall refusal. The implementation specification accepts frozen-until-Retry behavior, not the incorrect advice. The
rare-diagnostic filter requires rarity, which is not established for this normal sequence. No exact other coverage was
found. Proposed drop: none; this corrects the initial reviewer's filtering recommendation.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, discovery `g085/c2`.
