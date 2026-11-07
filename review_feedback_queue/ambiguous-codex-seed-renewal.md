# An uncertain Codex seed upload can replay after renewal

Reviewed commit: d5b4a317286682edac7e3dd402f981fae8b50410

## TLDR

Unverified after investigation: a saved Codex login can be installed again after a successful upload whose reply or
local success record was lost. If Codex renewed that remote login before the retry, replay could replace newer bytes.
Whether this makes authentication or later refresh unusable has not been established.

## Details

Suggested bucket: **other** for the established mechanism; potentially **highest** if the credential-loss route is
established. Confidence: confirmed local install-before-stamp ordering, with vendor renewal and invalidation unverified
after investigation. Retained at `aebf561680cc851498c6619d28b1be1ed3600617` and pinned tip
`49ce8cb76e9f4c841e15122a89264c7c908171dc`.

`crates/farhelm-helm/src/provisioning/credential_delivery.rs:503–518` resolves saved bytes and identity, compares the
local name/revision/incarnation stamp at `519`, installs at `543–548`, and records success only at `549–550`. That
branch has no remote-file read or generation comparison. A lost acknowledgement or a crash before the local stamp can
permit reinstallation. A committed matching stamp skips it across ordinary reconnects and survives Endpoint/None;
ordinary acknowledged reconnects clobbering a refreshed file are not alleged.

The governing sections and delivery branch were inspected at the original and pinned revisions, including original
SPEC.md `367–420`, SPEC_impl.md `5880–5985`, pinned SPEC.md `307–470,1937–1970`, SPEC_impl.md `5894–5983`, and delivery
`350–550`. Those checks establish replay ordering but not the vendor protocol. SPEC.md `409–413` assigns later
`auth.json` refresh to Codex; its rotating-copy warning at `1954–1959` does not prove renewal and invalidation in this
exact retry sequence. The missing premise is vendor renewal between the ambiguous upload and retry, followed by
restoration of the saved seed making authentication or later refresh unusable. Farhelm's opaque-byte handling cannot
settle that. A supported-version vendor source contract or an authorized isolated reproduction of the exact sequence
would. No actual credential file, vendor session or runtime experiment was used.

Possible cover: SPEC_impl.md `5930–5935` (original `5919–5924`) explicitly accepts uncertain completion and installation
that is not exactly once. The independent delivery consultation agrees that this covers ambiguity and repeat
installation, but does not explicitly accept the full conditional loss of usable credentials. Preserve that remainder.
It also explicitly agreed to **other** under the queue's rule that an unestablished source premise does not count toward
severity; evidence of renewal followed by harmful seed restoration moves it to highest regardless of rarity. No exact
plan, known bug, queue, ledger or filter coverage was found for that remainder.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g009/g009-f03` with duplicate
`g010/g010-auth-ambiguous-success`. Their original potentially highest consequence remains preserved. Proposed drop:
partial specification coverage only; the uncertain consequence is retained for triage, without claiming credential loss.
