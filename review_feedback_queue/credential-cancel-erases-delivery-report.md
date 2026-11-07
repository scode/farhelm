# Cancelling a credential draft erases the previous delivery report

Reviewed commit: 49ce8cb76e9f4c841e15122a89264c7c908171dc

## TLDR

After replacing a credential reports failed delivery to a host, opening and cancelling an unrelated credential draft
removes that report from Credentials. The host's separate warning remains; no new delivery attempt has superseded the
overview result.

## Details

Suggested bucket: **other**. Confidence: confirmed source behavior; no runtime experiment. Introduced by
`cffb06a42563389f4c281c26e50d2057363cac0b`.

`crates/farhelm-ui/src/credentials_settings.rs:128,153–155` retains and displays overview failures. Its `on_done`
handler replaces the failure vector unconditionally (`203–205`), while Back and Cancel pass an empty vector without
making a request (`338–339,403–404`). The delivery browser regression checks clearing after another save, not after
cancellation (`e2e/tests/credential-delivery.spec.ts:9–11,47–55`).

SPEC.md, Credentials, `1955–1956` requires this overview failure report; Topology, `385–394` describes failed pushes and
retries, and Settings, `1944–1945` preserves outcomes across navigation. Cancellation is not a successful later
delivery. Preserve the earlier result on Back/Cancel by distinguishing editor dismissal from save completion.

The host warning, amber status and host details persist independently
(`crates/farhelm-helm/src/hosts.rs:714–717,5390–5391`). This handler does not cause a failed delivery, and no
authentication outage, credential loss or security consequence is established from clearing the inline report. The
trigger is ordinary cancellation, so the rare-diagnostic filter does not fully match. No exact specification acceptance,
planned repair, queue item or ledger coverage was found.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g004/g004-warning-cancel`. Proposed drop: none.
