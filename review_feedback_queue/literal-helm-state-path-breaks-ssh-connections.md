# Literal `${...}` in the helm state path breaks SSH connections

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A literal environment-reference spelling in the state path breaks SSH routing.

## Details

F127 — **definite** — `crates/farhelm-helm/src/ssh.rs:365` — Literal `${...}` in the helm state path breaks SSH
connections

The sharing-path encoder leaves OpenSSH's `${...}` expansion syntax active. A literal state directory containing that
spelling is therefore interpreted as an environment reference, producing an error or a different control-socket path.
Supervisor connections and provisioning can remain unusable across retries. Reject active expansion syntax from the
sharing path or use the existing no-sharing fallback whenever the path cannot be represented faithfully.

## Evidence and triage context

- crates/farhelm-helm/src/ssh.rs:361–369 escapes backslashes, quotes and percent signs, leaving ${NAME} unchanged inside
  ControlPath.
- crates/farhelm-helm/src/ssh.rs:213–254 falls back only for socket length; otherwise it passes the encoded path with
  ControlMaster=auto.
- crates/farhelm-helm/src/transport.rs:113–119 supplies the helm state directory to supervisor SSH connections;
  provisioning/backend.rs:564–568 uses the same encoder for provisioning.
- /usr/share/man/man5/ssh_config.5.gz:687–701,2391–2409 documents ControlPath environment expansion. Retained run
  d87af1cf-f949-425b-b275-936c3f824f73 contains the matching generated option and an absent-variable exit 255 with
  'invalid environment variable expansion'.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:3764–3784 covers excessive socket-path length. Its trigger and remedy do not cover environment
  expansion in a short path.

Caveats:

- Requires a state path containing literal ${...} and short enough to enable sharing.
- A defined variable can redirect the socket path without necessarily preventing connection.
- The retained offline reproduction was inspected, not rerun.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_09_cor:p1:F1`.

- `helm_state_provisioning_09_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
