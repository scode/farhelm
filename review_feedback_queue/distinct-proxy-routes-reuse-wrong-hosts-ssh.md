# Distinct proxy routes can reuse the wrong host’s SSH connection

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

SSH connection sharing can send provisioning commands to the wrong machine.

## Details

F8 — **definite** — `crates/farhelm-helm/src/ssh.rs:174` — Distinct proxy routes can reuse the wrong host’s SSH
connection

The SSH sharing key includes resolved host, user, port, and jump-host information, but omits a custom proxy command. Two
aliases with different proxy routes can therefore share an existing connection when those other inputs match. Fresh-host
provisioning has no recorded installation identity to detect the wrong machine, so confirmation can lead to changes on
another destination. Include enough destination and routing identity in the sharing key, preserving resolved-target
discrimination and the socket-length fallback.

## Evidence and triage context

- crates/farhelm-helm/src/ssh.rs:174–178 names sockets solely %C or p%C; lines 249–258 enable automatic reuse before
  supplying the destination.
- /usr/share/man/man5/ssh_config.5.gz:2293–2294,2320–2322 defines %C as a hash of %l%h%p%r%j; ProxyCommand is absent.
  The inspected retained configuration probe produced identical paths for different ProxyCommand routes.
- crates/farhelm-helm/src/provisioning/backend.rs:558–575 applies this sharing prefix to remote provisioning commands.
- crates/farhelm-helm/src/provisioning/service.rs:603–634 creates an ADD plan after absence and reach inspection. Lines
  1655–1668 re-probe through the same route, and line 1728 permits execution if absence persists.
- crates/farhelm-helm/src/provisioning/service.rs:1637–1640 executes the confirmed plan after that revalidation.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:1149–1163 prescribes %C sharing, and TRIAGE_OUTCOMES.md:3764–3784 preserves it for resolved-target
  identity and socket length. Neither accepts different ProxyCommand destinations sharing a master or provisioning the
  wrong machine.

Caveats:

- Requires matching effective host, user, port and ProxyJump inputs, differing ProxyCommand routes, and an existing
  reusable master.
- Different ProxyJump routes are distinguished by the documented hash on the inspected OpenSSH version.
- Existing installation identities can cause some connections to refuse; fresh-host ADD remains exposed.
- No live two-host installation was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_09_sec:p1:F1`.

- `helm_state_provisioning_09_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
