# Older handoff reports success for a directory program

Reviewed commit: 918eea3a7613d74fd8deb2886ced1d5c542626d3

## TLDR

Installing an older release can report success while leaving an already damaged app unlaunchable because its
main-program path is a directory.

## Details

Production `scripts/install.sh:940–975` executes the older release's own installer. Published v0.27.0 bytes match the
reviewed fixture. That script checks parent directories (`scripts/fixtures/install-v0.27.0.sh:1182–1194`) but copies
each program using `mv -f` (`446–451,1257–1268`). If the exact destination is a directory, the new file is placed inside
it. Installed advances and the script prints success (`1435–1437`), although the app cannot execute that directory.

Suggested bucket: `high`. Confidence: confirmed conditional source composition and published-source correspondence;
occurrence unmeasured. The app is already damaged at entry. No newly bricked healthy app, process loss or unrelated-file
deletion was established. A symlink-destination variant remains conditional on an unestablished harmful outside-work
route; that stronger possibility is not declared safe.

Possible cover: trailing-tip ledger `TRIAGE_OUTCOMES.md:10345–10370` records the modern directory-destination repair,
not acceptance of exposure through historical handoff. `SPEC_impl.md:4612–4622` requires the handoff without explicitly
accepting false success. The rare-failure filter excludes success reported for a failed operation.

Provenance: `g017/f3`, independently retained by `a002`. Modern malformed-destination tests do not cover releases below
the historical-handoff floor. No tests or reproduction ran.
