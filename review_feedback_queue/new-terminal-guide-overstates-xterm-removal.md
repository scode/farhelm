# The migration guide overstates removal of xterm.js

Reviewed commit: f08c2cc83aec94260e865270e2cd57b41a2cf528

## TLDR

The new-terminal guide says no xterm.js is involved, although it still initializes the viewer and determines the size
used for attachment. This is an inaccurate architectural explanation, with no established functional failure.

## Details

Suggested bucket: **other**. Confidence: **confirmed by source**. Proposed drop: none. Possible cover: a charitable
reading restricted to live rendering is true, but the actual wording makes a broader claim.

`website/src/content/docs/docs/using/new-terminals.md:11–12` says "no xterm.js is involved either". At the reviewed
commit, `crates/farhelm-ui/assets/terminal.js:3844,3937–3939,4014–4015` constructs, opens and fits xterm. Attachment at
4076–4079 uses its dimensions; `adoptGrid` at 5299–5397 creates the grid before disposing xterm at 5365. SPEC_impl.md,
"The grid client", 1706–1736 explicitly describes this initialization. The absolute guide wording and the sizing route
survive through c5ae8548439cc5bfa33c11ef7ce8da6129d8072a.

SPEC.md's "Documentation scope" simplification exception covers the README introduction and Overview, not this migration
guide. The historical release notes use transitional wording and preserve the old-terminal dependency; they do not
supply a matching correction or exception. Say that live terminal display uses the grid, rather than claiming no xterm
involvement. No implementation change is implied.

Targeted coverage checks found no matching Planned item, unlanded plan, BUGS entry, queue record or triage decision. The
plans queue and TODO Planned are empty. No filter establishes an exclusion for this architectural explanation.
Provenance: review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g023/g023-c1`. Source-only; no browser or build ran for
this finding.
