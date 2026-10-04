# App updates can overwrite files outside a symlinked bundle directory

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

App updates can overwrite files outside a symlinked bundle directory.

## Details

`F26 / COR-INSTALLER-SYMLINK` — **possible** — `scripts/install.sh:1176` — App updates can overwrite files outside a
symlinked bundle directory

Updating an app with a recognized installation record can overwrite files outside `Farhelm.app` if one of the app's
directories is a symlink. The installer checks that `Contents/Versions` itself is a real directory, but does not make
the same check for the app root, `Contents`, `MacOS` (which holds the programs), or `Resources` (which holds the icon).
Its `mkdir -p` accepts directory symlinks, and its file-replacement helper copies a temporary file beside the
destination, then renames it over the destination. A linked `Resources` directory can therefore redirect replacement of
`Farhelm.icns` into another directory; linked program directories or ancestors can do the same for other files.

The earlier whole-bundle replacement did not write through these internal directory links. Check that the app and every
directory traversed by an in-place update have the expected real-directory shape before writing, and refuse redirected
paths. Regression fixtures should put sentinel files behind each linked ancestor and verify that refusal preserves them.

Suggested bucket: highest

Possible cover: `TRIAGE_OUTCOMES.md:2020–2047` accepts installation records as evidence of app ownership, and
`SPEC.md:1843–1849` excludes deliberate interference by processes under the same Unix account. Neither explicitly
settles accidental directory redirection. This concerns an already altered directory layout, not containment against a
hostile process using the same account.

Caveats: The mechanism follows from source inspection; the whole installer has not been run to reproduce it. An
accidental symlinked layout remains an unverified premise.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_general p1`, `cli_lifecycle p1`, `cli_systems p1`, `cli_edges p1`,
`cli_security p1`, `cli_trust p1`, `cli_secrets p1`, `cli_data p2`.

Possible cover recorded during collection: Recorded-app ownership: TRIAGE_OUTCOMES.md:2020–2047; deliberate local
interference: SPEC.md:1843–1849; no explicit accidental redirected-directory acceptance..

Collection caveats: No whole-installer runtime repro; accidental static shape, not same-user containment claim.

Merge calibration: several lenses filed definite source mechanisms; a possible input was merged at this same location.
The retained confidence is the lower possible tag.

## Filed reviewer metadata

- `cli_general p1`: confidence as filed: definite / confirmed by code and the ownership contract. proposed_drop: none.
  possible_cover: none. SPEC.md:288–293 extends the no-foreign-file-destruction rule to installation; SPEC.md:348–350
  forbids redirected removal. The shared-writable-directory exclusion in SPEC.md:2011–2014 does not cover a same-user
  bundle directory pointing into another private directory. Suggested bucket as filed: highest. Confidence: definite /
  confirmed by code and the ownership contract. proposed_drop: none. possible_cover: none. SPEC.md:288–293 extends the
  no-foreign-file-destruction rule to installation; SPEC.md:348–350 forbids redirected removal. The
  shared-writable-directory exclusion in SPEC.md:2011–2014 does not cover a same-user bundle directory pointing into
  another private directory.
- `cli_data p2`: confidence as filed: definite / confirmed. The required premise is a recognized current-layout
  installation whose MacOS or Resources directory has been replaced by a symlink to an existing directory. The path
  checks do not preclude this, and no timing race is required. Suggested bucket as filed: highest.
- `cli_lifecycle p1`: confidence as filed: definite / confirmed for the filesystem behavior; no runtime reproduction.
  Suggested bucket as filed: `highest`.
- `cli_systems p1`: confidence as filed: `definite`; `confirmed` by code and the ownership contract, conditional on the
  stated filesystem layout. No runtime reproduction. Suggested bucket as filed: `highest`.
- `cli_edges p1`: confidence as filed: definite/confirmed for an existing recorded installation whose Resources or MacOS
  directory has been replaced by a symlink to a real directory containing the corresponding basename. This is an unusual
  filesystem state, not a reproduced failure. Suggested bucket as filed: highest (user-file loss).
- `cli_security p1`: confidence as filed: definite / confirmed. The premise is an existing genuine recorded installation
  whose `Contents/MacOS` or `Contents/Resources` directory has been replaced by a symlink to another user-owned
  directory. No race, forged record, different account, or malicious release is needed. Suggested bucket as filed:
  highest (unintended loss of unrelated files).
- `cli_trust p1`: confidence as filed: possible; likely if a modified app with a valid record and symlinked internal
  directory remains inside the ownership guarantees. The path to overwrite is confirmed. Suggested bucket as filed:
  highest.
- `cli_secrets p1`: confidence as filed: definite / confirmed conditional on a pre-existing symlinked component in an
  installer-owned app. No runtime reproduction. Suggested bucket as filed: highest.
