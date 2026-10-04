# Repairing a partial app removes unrelated staging-like files

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Repairing a partial app removes unrelated staging-like files.

## Details

`F32 / COR-INSTALLER-STAGING-GLOB` — **possible** — `scripts/install.sh:1177` — Repairing a partial app removes
unrelated staging-like files

The installer now accepts an app with a valid installation record and a `Versions` directory even when its
`Info.plist`—the app metadata file—is missing. Before repairing it, cleanup deletes every regular, non-symlink file
matching `.farhelm-new.*` in several app directories. That includes a user file such as `.farhelm-new.notes`, although
the installer never created it. The shared prefix is the only ownership test.

This finding concerns the newly accepted partial-app shape. The baseline installer refused an app without `Info.plist`;
it therefore did not reach this deletion. Preserve uncertain filename collisions, and establish ownership of the exact
staging artifact before removing it.

Suggested bucket: highest

Possible cover: none identified for this newly accepted repair shape.

Caveats: A partial bundle containing a foreign staging-like file has not been reproduced. An intact bundle that the
baseline already replaced wholesale is excluded from this finding. The independent audit classified the ownership record
as ambiguous.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_secrets p1`, `cli_trust p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Conditional partial-bundle foreign file unverified; intact successful update already deleted bundle
at base, excluded variant. D36 audit ambiguous-record.

## Filed reviewer metadata

- `cli_trust p1`: confidence as filed: possible; likely only if the staging-name family is not an explicitly reserved
  ownership namespace. Deletion is confirmed. Suggested bucket as filed: highest.
- `cli_secrets p1`: confidence as filed: definite / confirmed conditional on a regular user-owned file matching the
  glob; no known user incident or runtime reproduction. Suggested bucket as filed: highest.
