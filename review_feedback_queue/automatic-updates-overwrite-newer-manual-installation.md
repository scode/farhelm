# Automatic updates can overwrite a newer manual installation

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A delayed automatic update can undo a newer manual installation.

## Details

F102 — **definite** — `crates/farhelm-ui/src/desktop/updater.rs:791` — Automatic updates can overwrite a newer manual
installation

The updater chooses a release before taking the installation lock. A newer manual installation can finish during that
interval, after which the automatic installer acquires the lock and publishes its older selected release. The next
launch can therefore reverse a successful upgrade or prerelease choice. Revalidate automatic-update eligibility under
the installation lock immediately before replacement, while preserving intentional manual version pinning.

## Evidence and triage context

- crates/farhelm-ui/src/desktop/updater.rs:752 reads Installed before the version comparison at :770 and install call at
  :791.
- crates/farhelm-ui/src/desktop/updater.rs:976 downloads the verified installer; :1014 supplies the previously selected
  version.
- scripts/install.sh:1140 acquires the mutation lock; :1236 reads the previous installed version for bookkeeping; :1257
  and :1264 replace the executable and Installed record without checking version precedence.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- No matching Planned, BUGS, queue, or filter coverage. SPEC_impl.md:3273 describes withholding automatic updates while
  a newer Installed version waits. TRIAGE_OUTCOMES.md:5475 concerns remote-host downgrade prevention, a different
  installation path.

Caveats:

- Inspection only; not reproduced on macOS.
- The manual installation must complete and release its lock before the background installer acquires it.
- Confirmed consequence is software downgrade. Additional session or data loss was not established.
- Inspection only; no macOS reproduction.
- Confirmed consequence is a downgrade of the next-launch version, not established session or data loss.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_06_sec:p1:F1`.

- `ui_desktop_06_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
