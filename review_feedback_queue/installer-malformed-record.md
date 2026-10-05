# Malformed app records may authorize foreign bundle replacement

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Malformed app records may authorize foreign bundle replacement.

## Details

`F33 / COR-INSTALLER-MALFORMED-RECORD` — **possible** — `scripts/install.sh:405` — Malformed app records may authorize
foreign bundle replacement

The fallback intended to recognize an installation whose Terminal-link location has moved does not require a correctly
encoded installation record. A real record contains exactly two NUL-terminated fields: `farhelm-app-v2` and an absolute
path to the Terminal command. This fallback converts NUL bytes to newlines and checks only the first two resulting
lines. A newline-only lookalike, or a record with extra fields, can therefore pass if those lines contain the expected
identifier and an absolute path ending in `/farhelm`.

Passing this check authorizes changes to the app. If it lacks a real `Versions` directory, the installer rebuilds it and
deletes the old bundle after replacement. A malformed marker in a foreign or customized app could thus authorize
destruction. Require the exact two-field NUL-terminated encoding, validate the link path, and compare the complete bytes
with the expected record before granting replacement authority.

Suggested bucket: highest

Possible cover: none identified.

Caveats: The permissive parser and destructive rebuild path are visible in the source. A foreign or customized app
containing such a malformed lookalike is an unverified practical premise, and the audit of that premise remains pending.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_lifecycle p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Foreign/custom app malformed lookalike practical premise unverified; audit pending.

Audit update after restatement: independent audit D51 retained this as ambiguous, possible highest. Valid-record
recognition does not expressly cover malformed lookalikes. The practical malformed-record premise remains unverified.

## Filed reviewer metadata

- `cli_lifecycle p1`: confidence as filed: possible / likely. Acceptance is code-established; the material premise is
  that a foreign or customized app carries a malformed lookalike record rather than a genuine installer record.
  Suggested bucket as filed: `highest` pending independent review.
