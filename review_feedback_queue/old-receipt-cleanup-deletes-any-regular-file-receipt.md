# Old-receipt cleanup deletes any regular file at the receipt name

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Old-receipt cleanup could delete unrelated content at the reserved receipt path.

## Details

F25 — **possible** — `scripts/install.sh:1355` — Old-receipt cleanup deletes any regular file at the receipt name

The installer rejects an unrecognized old receipt when deciding ownership of binaries, yet later deletes any regular,
non-symlink file at that receipt pathname. If legitimate unrelated content occupies the path, installation or update
removes it without ownership evidence. No ordinary producer of such content has been identified, so that premise remains
uncertain. Recognize a genuine obsolete receipt before deleting it; retain or report an unrecognized file.

## Evidence and triage context

- scripts/install.sh:548–554 treats a nonmatching receipt magic as no usable ownership record when deciding whether old
  binaries belong to the installer.
- scripts/install.sh:1354–1355 nevertheless deletes every regular, non-symlink file at the old receipt pathname without
  checking that magic or whether any old installation was recognized.
- scripts/install.sh:1293 publishes a fresh bundle before reaching the same receipt cleanup, so this is not confined to
  migration of a recognized old installation.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2160–2164 excludes deliberately crafted interference, but whether legitimate unrelated content can occupy this
  pathname remains unresolved.
- SPEC.md:337–341 and TRIAGE_OUTCOMES.md:6693–6706 reserve recognized bundle contents and named temporary files. The old
  bin-directory receipt is outside the bundle and is neither listed temporary name.
- SPEC.md:330–335 states that matching an installer filename alone does not authorize destroying a file.

Caveats:

- The unverified premise is legitimate unrelated user-owned content at ~/.local/bin/.farhelm-installation.
- No ordinary Farhelm operation producing that unrelated content was identified.
- A valid obsolete installer receipt is expected cleanup and is not the finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_10_sec:p1:F6`.

- `cli_installation_10_sec:p1:F6`: confidence as filed: possible; suggested bucket as filed: highest.
