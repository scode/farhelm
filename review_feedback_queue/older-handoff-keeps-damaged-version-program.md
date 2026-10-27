# Older handoff keeps a damaged version program while reporting ready

Reviewed commit: 918eea3a7613d74fd8deb2886ced1d5c542626d3

## TLDR

Reinstalling v0.27.0 can report that Farhelm is ready while retaining its already damaged command-line executable, even
though the installer downloaded verified replacement bytes.

## Details

Production handoff at `scripts/install.sh:940–975` reaches the published historical installer, whose correspondence to
the fixture was verified. `scripts/fixtures/install-v0.27.0.sh:1239–1256` reuses an existing version's `farhelm`
whenever it is a regular non-symlink file, including an empty or otherwise damaged one. It verifies the downloaded
program (`1078–1085`), but never compares that download with the reused file. Installed advances at `1257–1268` and
ready is printed at `1435–1437`.

Suggested bucket: `high`. Confidence: confirmed conditional source mechanism for an already damaged regular target. This
is failure to repair, not new damage to a healthy installation. No process/data loss or particular APFS damage-producing
sequence was established. Possible cover: trailing-tip `TRIAGE_OUTCOMES.md:18689–18707` explicitly requires repairing
this exact input and records a modern-installer fix. `SPEC_impl.md:4640–4648` requires verified-byte comparison. Those
repairs do not accept the old-target handoff gap; current shell repair tests use `1.2.3` above its floor. A current I/O
failure is unnecessary and success is false, so the filesystem-failure allowance does not fully cover it. Provenance:
`g019/f5`, followed by another search. No runtime reproduction ran.
