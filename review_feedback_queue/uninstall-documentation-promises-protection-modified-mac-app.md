# Uninstall documentation promises protection for modified Mac app files

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The uninstall guide promises Mac content protection the product does not provide.

## Details

F337 — **definite** — `docs/install_uninstall.md:150` — Uninstall documentation promises protection for modified Mac app
files

This is documentation-only; no implementation change is requested. The guide says changed contents cause refusal without
limiting that promise to Linux. Recognized macOS app files are checked for layout, type, ownership, and identity rather
than contents, so wanted customizations can still be removed. Qualify the checksum guarantee as Linux-only and explain
Mac removal behavior. Reliance and actual loss are conditional, while the documentation mismatch is definite.

## Evidence and triage context

- docs/install_uninstall.md:150 states the guarantee.
- crates/farhelm/src/uninstall/app.rs:174–185 schedules app files for removal; :269–285 uses open_regular.
- crates/farhelm/src/uninstall/ownership.rs:234–266 checks type, owner and identity; removal.rs:98–99 removes the files.
- SPEC_impl.md:2882–2890 explicitly omits checksums for this macOS behavior.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:337–341 accepts removing additions inside a recognized installer-owned app bundle. It covers behavior, not the
  contradictory documentation. TRIAGE_OUTCOMES.md:1994–2047 concerns foreign bundle recognition.

Caveats:

- Documentation-only. Actual loss requires customized contents and reliance on the wording. No runtime reproduction.
  Original reviewer marked highest_possible for that consequence.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_04_cor:p2:F1`.

- `automation_website_04_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
