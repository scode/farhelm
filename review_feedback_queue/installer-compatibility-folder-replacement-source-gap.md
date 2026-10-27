# Compatibility-lock cleanup retains a replacement-folder source gap

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

Cleanup renames the current compatibility-lock folder before removing it. A supported replacement causing harmful
movement of unrelated work remains unestablished.

## Details

`crates/farhelm-supervisor/src/installation/mod.rs:304–323,353–366,410–417` renames the current fixed name, then uses
nonrecursive `REMOVEDIR`. It does not recursively delete nonempty contents. The original uninstaller caller
(`crates/farhelm/src/uninstall/locks.rs:151–179`) creates only an empty directory and refuses an existing name.
Occupying the stale-removal/publication gap makes `NOREPLACE` refuse. Later shared-kernel-lock callers preserve that
protection, but establish no foreign replacement producer.

Suggested bucket: `other`. Confidence: confirmed cleanup shape with missing ordinary replacement and impact premises.
Possible cover: trailing-tip `SPEC.md:3320–3333` covers deliberate marker reshaping only. The
nonempty-recursive-deletion allegation is source-refuted; the wrong-target rename remainder is not excluded. Provenance:
`g013/f1`, `a003/f4`. No runtime experiment ran.
