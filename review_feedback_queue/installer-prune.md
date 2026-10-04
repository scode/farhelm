# Old version cleanup can delete foreign user contents

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Old version cleanup can delete foreign user contents.

## Details

`F29 / COR-INSTALLER-PRUNE` — **possible** — `scripts/install.sh:1337` — Old version cleanup can delete foreign user
contents

The app keeps older Farhelm command binaries in version-named directories under `Contents/Versions`. At the end of an
update, the installer retains the new version, the previously installed version, and versions named by the
running-supervisor records it checks. Any other real directory with a release-like name is recursively deleted, without
checking whether it contains only an installer-created binary. A user-added note inside an old version, or an unrelated
directory named like a release, is therefore deleted with it.

Before pruning, establish that a directory contains only artifacts the installer owns; retain or refuse directories with
extra contents. Add a sentinel-file regression for the uncertain ownership case. This follows the general rule in
`SPEC.md:288–293` that a matching name alone does not authorize destruction, and matches uninstall's stricter refusal of
extra entries, including extra files inside version directories (`crates/farhelm/src/uninstall/app.rs:551–574`).

Suggested bucket: highest

Possible cover: `SPEC_impl.md:2904–2907` explicitly prescribes version-folder pruning, and
`TRIAGE_OUTCOMES.md:2033–2044` recognizes app ownership through its record. The earlier whole-bundle replacement and the
decisions around `TRIAGE_OUTCOMES.md:2018–2045` may also cover deletion within a recognized bundle. They do not
expressly settle foreign extra contents.

Caveats: Deletion of extra contents is supported by inspection, conditional on those contents existing. Whether the
accepted bundle-ownership and pruning policies already authorize it remains ambiguous; the independent audit also
classified that record as ambiguous.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_general p1`, `cli_data p1`, `cli_edges p1`, `cli_security p1`, `cli_systems p1`,
`cli_lifecycle p1`, `cli_trust p1`, `cli_secrets p1`.

Possible cover recorded during collection: SPEC_impl.md:2904–2907 prescribes version-folder pruning;
TRIAGE_OUTCOMES.md:2033–2044 record-based bundle recognition. Foreign extras are not explicitly settled..

Collection caveats: Conditional userextra contents; deletionconfirmed but ownership policy ambiguous. IndependentD29
ambiguousrecord; possiblecover SPEC_impl2904–2907 prescribedversionpruning/ledger2018–2045 oldbundle replacement.

## Filed reviewer metadata

- `cli_general p1`: confidence as filed: definite / confirmed conditional on user content inside an unprotected
  version-named directory. proposed_drop: explicitly prescribed pruning in SPEC_impl.md:2904–2907, and the broader
  pre-existing deletion above. possible_cover: SPEC_impl.md:2904–2907; scripts/test-install-sh.sh:the version-pruning
  cases within 760–1085 preserve the new, replaced, and Running versions and remove older version directories. Suggested
  bucket as filed: highest. Confidence: definite / confirmed conditional on user content inside an unprotected
  version-named directory. proposed_drop: explicitly prescribed pruning in SPEC_impl.md:2904–2907, and the broader
  pre-existing deletion above. possible_cover: SPEC_impl.md:2904–2907; scripts/test-install-sh.sh:the version-pruning
  cases within 760–1085 preserve the new, replaced, and Running versions and remove older version directories.
- `cli_data p1`: confidence as filed: definite / confirmed: a real version-named directory containing an extra file is
  recursively removed once it is outside the protected versions. No filesystem error or race is required. Suggested
  bucket as filed: highest.
- `cli_lifecycle p1`: confidence as filed: definite / confirmed for deletion; whether files added inside an
  installer-owned version should count as protected user work is the contract question. Suggested bucket as filed:
  `highest` pending independent drop review.
- `cli_systems p1`: confidence as filed: `possible`; `likely`, with the material coverage premise below unresolved. The
  deletion behavior itself is confirmed by code. Suggested bucket as filed: `highest`.
- `cli_edges p1`: confidence as filed: definite/confirmed deletion if a prunable real version directory contains a user
  file; the open premise is whether the specified app ownership/pruning policy grants authority over all descendants,
  including added foreign files. Suggested bucket as filed: highest (possible user-file loss).
- `cli_security p1`: confidence as filed: possible / likely for a contract defect; confirmed for the recursive deletion.
  Open premise: whether an extra user file inside a recorded app's version folder remains independently protected by the
  ownership rule. Suggested bucket as filed: possibly highest (user files).
- `cli_trust p1`: confidence as filed: possible; likely conditional on the ownership rule protecting extra files inside
  an installer-built app. The recursive deletion itself is confirmed by code. Suggested bucket as filed: highest.
- `cli_secrets p1`: confidence as filed: definite / confirmed conditional on a user-owned version-named directory or an
  extra user file inside an old version. No runtime reproduction. Suggested bucket as filed: highest.
