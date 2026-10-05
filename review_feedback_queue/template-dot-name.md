# Dot-only template names pass validation but cannot be saved

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Dot-only template names pass validation but cannot be saved.

## Details

`F57 / COR-TEMPLATE-DOT-NAME` — **definite** — `crates/farhelm-ui/src/api.rs:2938` — Dot-only template names pass
validation but cannot be saved

The Templates panel lets the user enter `.` or `..` as a template name, and the shared name validation accepts both.
Saving puts the name in the final segment of the request URL. Although the encoder turns the dots into `%2E`, the URL
parser treats encoded dot-only segments as path navigation and removes them before dispatch. The request therefore
misses the named-template route and the otherwise valid-looking draft cannot be saved.

Reject dot-only template names in shared validation and explain the refusal before sending, or carry the name somewhere
other than a path segment that undergoes normalization. Verify the URL at the actual request boundary. The same URL
construction is used for deleting an existing template, so a dot-only name imported through another path would also have
a deletion problem.

Possible cover: `TRIAGE_OUTCOMES.md:6332–6350` covers dot-only session IDs, not template names. The review filter for
rare, safely failed operations may be relevant, but the shipped panel accepts these names; the trigger has not been
established as rare or as input the client never sends.

Caveats: No browser or runtime reproduction, or mutation, was performed. Saving fails safely; no destructive alternate
route was identified. The existing-template deletion consequence requires a dot-only name to have already been imported.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `ui_data p1`.

Possible cover recorded during collection: TRIAGE_OUTCOMES6332–6350 covers dot-only session IDs, not templates. FILTER
rare safe failures may be considered, but shipped settings accepts these names and rare trigger has not been
established..

Collection caveats: No browser/runtime reproduction or mutation. Safe failed save. Existing-name deletion consequence
requires such a name already imported; no destructive route identified.

## Filed reviewer metadata

- `ui_data p1`: confidence as filed: **definite / confirmed**. Premise: the shared template-name validator accepts `.`
  and `..`, and WHATWG URL parsing normalizes percent-encoded dot segments. Suggested bucket as filed: **other**.
