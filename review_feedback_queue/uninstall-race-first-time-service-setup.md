# Uninstall can race first-time service setup

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Concurrent setup and uninstall can leave services pointing at a deleted executable.

## Details

F95 — **definite** — `crates/farhelm/src/setup.rs:417–423` — Uninstall can race first-time service setup

First-time setup can publish service definitions after uninstall has determined that no unit directory needs exclusion.
Uninstall then removes the executable, while setup leaves new definitions referring to it. Resolving the executable
earlier does not reserve it against removal. Give setup and uninstall shared exclusion even before the unit directory
exists, or refuse the conflicting transition with an equivalent guard.

## Evidence and triage context

- setup.rs:417–423 omits exclusion when the selected services and unit directory are absent. uninstall.rs:250–261 takes
  the install lock and this optional setup lock; setup instead canonicalizes its executable at setup.rs:605–606 and
  takes only the unit-directory lock at :690–694. After uninstall's final service recheck at uninstall.rs:299–309, setup
  can create the directory and publish units at setup.rs:750 while uninstall removes the executable at
  uninstall.rs:183–186.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- SPEC.md:406–427 requires correct overlapping outcomes; its conditional unit-directory locking at :409–413 does not
  explicitly accept this outcome. TRIAGE_OUTCOMES.md:4204–4216 discards harmless leftover lock files, not missing
  exclusion. provisioning-update-replaces-binary-before-setup-guard.md concerns remote provisioning and a different
  mutation site.

Caveats:

- Requires overlapping commands on a Linux flat installation with no initial unit directory.
- No runtime reproduction or established loss of running user processes.
- Setup may itself report a service-start failure; that does not prevent uninstall from reporting success while leaving
  service definitions behind.
- Requires overlapping commands on a Linux flat installation with no initial unit directory. No runtime reproduction or
  user-process loss established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_02_cor:p1:F1`.

- `cli_installation_02_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
