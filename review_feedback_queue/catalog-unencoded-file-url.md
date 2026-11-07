# Catalog import confuses filesystem delimiters with file-URL syntax

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The Tensorlake test runner cannot load its terminal catalog from a tested checkout whose directory name contains # or
certain literal percent escapes. It builds a URL from an unencoded filesystem path.

## Details

Source coordinates from the original records: `scripts/tensorlake/sandbox-tests.ts:132-134,218-219,263-269`. References
below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Later-turn source-only verification confirms the original claim. scripts/tensorlake/sandbox-tests.ts:31-33 derives REPO
from an on-disk directory, or 263-269 accepts --repo as that filesystem path. oldTerminalSet at 130-134 explicitly
promises to load the tested checkout's catalog, then interpolates Deno.realPathSync into a file:// import specifier.
realPathSync yields a filesystem spelling, not an encoded URL. Both default suite selections call it at 218-219; no
runtime, cloud or endpoint experiment was performed.

Accessible dependency source corroborates the conversion contract: cached @std/path 1.1.6 posix/to_file_url.ts:23-32
first creates file:/// and assigns pathname, explicitly replacing every literal percent sign with %25.
posix/from_file_url.ts:20-25 instead uses only url.pathname and decodeURIComponent; URL fragments are not filesystem
path text. Cached whatwg-url 17.2.0 URL-impl.js:172-182 separates pathname from fragment, and its
url-state-machine.js:971-999 ends ordinary path parsing at # and switches to the fragment state (also handles ? as
query). The pathname setter deliberately uses stateOverride rather than ordinary URL-string parsing. These were read as
source only; whatwg-url is corroborating URL-contract source, not claimed to be Deno's runtime implementation.

Consequently, a real path /checkout/review#one/e2e/suite-terminals.ts interpolated after file:// has pathname ending at
/checkout/review and the intended suffix in its fragment. A directory literally named review%20one is interpreted as
review one when resolving the URL path. URL encoding (for example @std/path toFileUrl) preserves the actual path. This
is not a claim that ordinary spaces are broken: URL parsing can encode spaces; the examples rely specifically on
delimiters and literal escapes. SPEC_impl.md Testing:7489-7496 describes the suite split; the runner's explicit
filesystem --repo contract supplies the path premise, with no special exclusion for these names.

Suggested bucket: other, maintainer tooling only. Confidence: confirmed from runner and dependency source contracts, no
runtime experiment. proposed_drop: false. possible_cover: none. Fix by converting the canonical filesystem path to a
file URL before dynamic import.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g006/g006-l3`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
