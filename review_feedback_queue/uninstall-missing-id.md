# Connected uninstall omits missing recorded identity comparison

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Connected uninstall omits missing recorded identity comparison.

## Details

`F9 / COR-UNINSTALL-MISSING-ID` — **possible** — `crates/farhelm-helm/src/provisioning/service.rs:1260` — Connected
uninstall omits missing recorded identity comparison

Before uninstalling a connected host, the helm checks its sessions over the existing connection, then opens a fresh
probe to verify the installation reached by the destination. The implementation specification requires the fresh
identity to match the recorded identity exactly, including whether either identity is missing.

The code compares identities only when the registry already contains one. The host manager permits a connection when
both the row and its peer have no identity. In that state, a fresh probe reporting a populated identity passes the
uninstall check without comparison. The session list and the installation selected for removal could therefore come from
different installations.

Require exact equality between the recorded and freshly reported optional identities, including the missing case. Add a
focused case with a connected identityless row and a fresh probe reporting an identity.

Suggested bucket: **highest**. No exact cover was identified. The comparison mismatch is confirmed, but
wrong-installation removal and the production reachability of the required identityless connection have not been
demonstrated. `TRIAGE_OUTCOMES.md:4139–4152` discarded an identityless **update** issue because its serving-peer premise
was unreachable; it does not establish exact acceptance for this uninstall check. The merged report records that drop
audit D13 disagreed with excluding this finding.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `helm_data p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Mismatch confirmed, wrong-install removal unverified; UPDATE ledger4139–4152 not UNINSTALL. Drop
audit D13 disagrees exclusion.

## Filed reviewer metadata

- `helm_data p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
