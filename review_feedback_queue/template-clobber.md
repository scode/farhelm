# A new template can overwrite saved choices while its catalog is unavailable

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

A new template can overwrite saved choices while its catalog is unavailable.

## Details

`F19 / COR-TEMPLATE-CLOBBER` — **definite** — `crates/farhelm-ui/src/list/templates.rs:414` — A new template can
overwrite saved choices while its catalog is unavailable

Creating a template can replace an existing template that the user never opened. The Templates panel fetches the saved
templates so it can refuse a duplicate name, but treats an unfinished or failed fetch as evidence that the name is
available. Save remains enabled. If the user enters an existing name while that read is slow or failing, and the save
request succeeds, the helm replaces all the fields stored under that name with the new draft.

This needs only one client and an ordinary read failure or delay. The implementation specification requires the panel to
refuse names already used by another template; the separate last-write-wins rule allows concurrent edits to the same
template and does not authorize this overwrite. Require a successfully read catalog at the moment a new template or
rename is saved, with a way to retry an unavailable catalog. A focused regression should hold or fail the catalog
request while attempting to save an existing name. Editing the template already open should remain replaceable as
intended.

Suggested bucket: highest

Possible cover: none

Caveats: Confirmed by source inspection; no runtime reproduction was performed.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `ui_general p1`, `ui_systems p1`, `ui_lifecycle p1`.

Possible cover recorded during collection: none identified.

Collection caveats: No runtime reproduction; same-template edits remain intentionally replaceable.

## Filed reviewer metadata

- `ui_general p1`: confidence as filed: definite / confirmed by the UI guard, create-or-replace API, and explicit panel
  contract. Suggested bucket as filed: highest. Confidence: definite / confirmed by the UI guard, create-or-replace API,
  and explicit panel contract.
- `ui_lifecycle p1`: confidence as filed: definite; confirmed by catalog-state and PUT semantics. Caveat: no browser
  reproduction was run. Suggested bucket as filed: highest (loss of saved user data).
- `ui_systems p1`: confidence as filed: definite; confirmed. Suggested bucket as filed: highest (loss of user-owned
  template data).
- `ui_edges p2`: confidence as filed: **definite / confirmed** by the resource-state conversion, enabled Save handler,
  and storage upsert. No runtime reproduction ran. Suggested bucket as filed: **highest**, because the consequence is
  durable loss of user-authored launch configuration.
