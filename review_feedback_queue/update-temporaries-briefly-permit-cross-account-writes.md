# Update temporaries briefly permit cross-account writes

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Update files briefly allow another account to retain write access.

## Details

F23 — **definite** — `scripts/install.sh:449` — Update temporaries briefly permit cross-account writes

Replacement files are created under the caller's permissions policy before being restricted and renamed into place. With
a permissive umask, traversable bundle path, and applicable write permission, another account can open a writable
descriptor during that gap and retain it after chmod and publication. Altered launch configuration or forwarding code
can then execute with the installer user's authority. Create sibling replacement files restrictively from the outset,
and apply final modes before publication.

## Evidence and triage context

- scripts/install.sh:332 creates the plist under the caller's umask.
- scripts/install.sh:449 copies it to a traversable destination directory before chmod at :450 and rename at :451.
- scripts/install.sh:1259 similarly replaces a changed generated forwarder; :1265 replaces the generated plist.
- crates/farhelm-ui/src/desktop/bundle.rs:51 honors the explicit supervisor-program override; :92 returns it
  unconditionally.
- crates/farhelm-ui/src/desktop.rs:395 executes the selected program.
- scripts/install.sh:449 creates the writable sibling temporary before chmod and rename.
- scripts/install.sh:1265 supplies the generated plist.
- crates/farhelm-ui/src/desktop/bundle.rs:92 and crates/farhelm-ui/src/desktop.rs:395 establish the executable-program
  override consequence.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:2102 addressed final plist mode, not retained descriptors opened before chmod. SPEC.md:2330
  excludes other-account directory-entry authority; this attack needs no such authority.
- Same location, trigger, authority boundary, consequence, and corrective mechanism as the retained input. The older
  final-mode triage decision does not fully cover either report.

Caveats:

- Requires a permissive umask and another account able to traverse the bundle path; group-write exploitation also
  requires the relevant group membership.
- No native macOS exploit reproduction.
- The forwarder is copied only when its contents change.
- Requires permissive umask, accessible directory traversal, applicable group/other permissions, and winning the opening
  window.
- The forwarder path applies when its contents change.
- Native macOS exploitation was not reproduced.
- Requires a permissive umask, shared group membership or equivalent write permission, directory traversal, and winning
  the temporary-file opening window.
- Native macOS launch-environment exploitation was not reproduced; verification was by source and filesystem semantics.
- Retain the permissive-umask, traversal, group-membership, and timing prerequisites.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_10_cor:p1:F1`,
`cli_installation_10_sec:p1:F1`.

- `cli_installation_10_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `cli_installation_10_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
