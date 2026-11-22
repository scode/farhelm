# Setup and takeover offer binding remains unverified

Reviewed commit: 40d1a549ffe9a1d72100154f59a1c1f9e74bd9ae

## TLDR

It remains unclear whether a setup answer always consumes the offer shown when the user confirmed, especially if the
draft changes while the offer is pending.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`crates/gui/farhelm-gui/src/hosts/add.rs:1256`.

add.rs:930–943,1000–1013 captures the sent values, 1016–1049 locks the fields for the request, and 1055–1114 preserves
those fields and replays the same command across sign-in. The synchronous settle re-enables them then captures the offer
(1187–1197,1218–1266). The takeover closure captures its probe ID (1223–1245). Setup submits the Offer used to draw its
dialog (setup.rs:258–354,523–531); Uninstall does likewise (uninstall.rs:301–339,357–366). A live launch-in-flight check
only vetoes the action. SPEC_impl.md:7908–7916 and historical 6ef88c9a:SPEC_impl.md:1551–1564 require binding to the
drawn answer. Bucket: other. Confidence: source-confirmed for these handlers. proposed_drop: refuted live substitution
for the scenario reviewed. Could be highest if a different destructive plan were sent, but the pinned handler receives
the offer it drew. Possible coverage: none.

The independent check did not settle the proposed exclusion. AMBIGUOUS via 40d1a549f:hosts/add.rs:928–945,1254–1271,
hosts/setup.rs:256–285, hosts/uninstall.rs:299–328, SPEC_impl.md:7906–7918. The reply snapshots live drafts into an
offer, and Setup derives its heading from that offer. The excerpts omit draft locking through settlement and the answer
callbacks that establish which Setup offer or takeover ID is submitted.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `provision/p8`.
