# Repairing an incomplete app version can discard user files

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Repairing an incomplete app version can discard user files.

## Details

`F30 / COR-INSTALLER-INCOMPLETE` — **possible** — `scripts/install.sh:1194` — Repairing an incomplete app version can
discard user files

When installing a version whose directory already exists but lacks a regular, non-symlink `farhelm` binary, the
installer moves that directory into its private update workspace and puts a complete version in its place. Cleanup then
recursively deletes the workspace, including the displaced directory. A user file inside that incomplete version is
consequently lost even though the installer never established ownership of its contents.

The failure path has the same concern: if placing the new version fails, the installer attempts to restore the displaced
directory but ignores a failed restore. Exit cleanup still deletes the workspace that holds it. Repair only a directory
shown to be installer-owned or empty; otherwise preserve and report the displaced contents. A failed restore must also
leave a reported recovery copy rather than erase it.

Suggested bucket: highest

Possible cover: `SPEC_impl.md:2898–2903` requires an interrupted update to converge on retry, and
`TRIAGE_OUTCOMES.md:2033–2044` accepts record-based bundle recognition. The previous whole-bundle deletion and the
decisions around `TRIAGE_OUTCOMES.md:2018–2045` may cover this too. The independent auditor did not consider that
coverage conclusive.

Caveats: The deletion mechanism is confirmed by inspection, conditional on an incomplete directory containing foreign
files. An ordinary interrupted installation does not create such extras. Ownership policy remains ambiguous, as recorded
by the independent audit.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_general p1`, `cli_data p1`, `cli_edges p1`, `cli_trust p1`, `cli_secrets p1`.

Possible cover recorded during collection: SPEC_impl.md:2898–2903 interruption convergence; TRIAGE_OUTCOMES.md:2033–2044
bundle recognition. Independent auditor disagreed with complete coverage..

Collection caveats: Deletionconditionalconfirmed, policyambiguity; ordinaryinterruption createsnoextras. IndependentD30
ambiguousrecord. Possiblecover basewholebundledeletion/ledger2018–2045+SPEC_impl2898–2903 retryconvergence.

## Filed reviewer metadata

- `cli_general p1`: confidence as filed: definite / confirmed conditional on the named folder containing user-owned
  extra files. proposed_drop: pre-existing broad bundle-content deletion; independent verification required.
  possible_cover: base scripts/install.sh:1203–1208,1266 replaced any Farhelm-looking bundle wholesale, already deleting
  extra contents. SPEC_impl.md:2830–2834 also treats a valid record as proof the bundle is this installation's; it does
  not explicitly grant ownership of arbitrary extra files. SPEC.md:288 remains a conflicting no-destruction-by-name
  contract. Suggested bucket as filed: highest. Confidence: definite / confirmed conditional on the named folder
  containing user-owned extra files. proposed_drop: pre-existing broad bundle-content deletion; independent verification
  required. possible_cover: base scripts/install.sh:1203–1208,1266 replaced any Farhelm-looking bundle wholesale,
  already deleting extra contents. SPEC_impl.md:2830–2834 also treats a valid record as proof the bundle is this
  installation's; it does not explicitly grant ownership of arbitrary extra files. SPEC.md:288 remains a conflicting
  no-destruction-by-name contract.
- `cli_data p1`: confidence as filed: definite / confirmed: an existing requested-version folder without a regular
  non-symlink farhelm is moved to the private work directory and that directory is subsequently removed recursively.
  Suggested bucket as filed: highest.
- `cli_edges p1`: confidence as filed: definite/confirmed if the target release's folder exists without a regular
  nonsymlink farhelm and contains unrelated user files; whether app ownership authorizes discarding that entire folder
  is the material policy premise. Suggested bucket as filed: highest (possible user-file loss).
- `cli_trust p1`: confidence as filed: possible; likely conditional on protection of extra contents in a recorded app.
  The move and subsequent deletion are confirmed. Suggested bucket as filed: highest.
- `cli_secrets p1`: confidence as filed: definite / confirmed conditional on an existing selected-version directory
  without a regular farhelm and with an unrelated entry. No runtime reproduction. Suggested bucket as filed: highest.
