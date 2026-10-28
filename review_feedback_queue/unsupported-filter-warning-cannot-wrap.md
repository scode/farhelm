# Unsupported-filter warning cannot wrap

Reviewed commit: ad333435c78161f39df8bfa0318994659dd590ae

## TLDR

A narrow session list can clip the explanation that an older helm ignored its filter. The full explanation remains in
the tooltip and accessible name.

## Details

Suggested bucket: **other**. Confidence: **confirmed CSS conflict and conditional producer**; the actual clipping
threshold and frequency of the older-helm pairing were not measured. No runtime reproduction was performed.

`crates/farhelm-ui/assets/app.css:1138–1141,1169–1171` gives `.session-heading .banner.session-count` the
`white-space: nowrap` rule with specificity `(0,3,0)`. Its intended warning override,
`.session-heading [data-warning="true"]`, has specificity `(0,2,0)` and loses. The conflict remains at pinned tip
`9b19f8d754bf7658761ef246cfda0ea8397f4f21`, lines `1224–1227,1255–1257`.

At that tip, `rows.rs:782–784,797` emits exactly those classes and the warning flag when a filtered reply has no
matching count. `api.rs:2395–2412` documents the older helm that ignores filtering. Truncated-list warnings use a
different class and are unaffected. Exclude warnings from the nowrap rule or make the warning override win.

Possible cover: FILTER.md's rare-diagnostic rule could cover a proven rare compatibility trigger, but that premise was
not established. SPEC.md:1619–1631 requires an explicit explanation. Searches found no exact Planned item, unlanded
cause-removing plan, BUGS entry, queue item or ledger decision. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g003/g003-f001`.
