# Template discovery failure looks like an empty catalog

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Template discovery failure looks like an empty catalog.

## Details

`F25 / COR-TEMPLATE-CATALOG-ERROR` — **definite** — `crates/farhelm-ui/src/list/create_form.rs:1830` — Template
discovery failure looks like an empty catalog

If the launcher cannot fetch saved templates, searching with `tl:` looks the same as having no templates. The fetch
converts its error into an empty list, so the user sees neither the failure nor a way to retry it. Saved choices
disappear from discovery even though the rest of the launcher remains usable.

Keep the fetch result's success or failure state. While `tl:` search is active, show that templates are unavailable and
offer a retry without blocking unrelated launcher choices. The product specification requires failed operations to
surface an actionable error. The nearby comment calling this read best-effort describes the implementation but does not
explicitly establish a product exception to that rule.

Suggested bucket: other

Possible cover: none

Caveats: No template data is lost. Closing and reopening the launcher fetches the templates again and can recover. No
runtime reproduction was performed.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `ui_general p1`, `ui_edges p1`.

Possible cover recorded during collection: none identified.

Collection caveats: No template data loss, reopening recovers. Nearby best-effort comment not explicit product
acceptance.

## Filed reviewer metadata

- `ui_general p1`: confidence as filed: definite / confirmed error-to-empty path. Suggested bucket as filed: other.
  Confidence: definite / confirmed error-to-empty path.
- `ui_edges p1`: confidence as filed: **definite / confirmed** from the resource and search path. Suggested bucket as
  filed: **other**.
