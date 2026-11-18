# An untagged leftover sandbox is labelled as another helm's

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

In Settings → Tensorlake's list of sandboxes not on this helm, a leftover whose name carries no helm tag at all (one
made by hand in Farhelm's reserved namespace, say) is labelled "from another helm", which says something the name does
not show.

## Details

Suggested bucket: **other**. Confidence: **confirmed from code** (by the independent drop check of the GUI migration's
final review, 2026-11-17, as a lead outside its register). Display wording only; the label errs toward warning.

`crates/farhelm-helm/src/tensorlake/orphans.rs:151` and `:210` set `from_another_helm = creator(&name) != Some(tag)`,
which is true when `creator` returns `None` for a name with no tag. SPEC.md (the Tensorlake leftovers paragraph around
lines 541-552) applies that label only "when its name carries another helm's tag". A fix shows the label only for
`Some(other)` tags, and either no label or a separate "no Farhelm tag" wording for `None`; the verification of the
related dropped finding (Tensorlake orphan cleanup can delete a manually created sandbox, dropped as SPEC-accepted)
suggested the latter.
