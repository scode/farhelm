# A link approval can arrive after its session is deleted

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

Deleting a session can finish before one of its link requests displays an approval card. The card can then remain for up
to nine minutes, even though its session is gone. Explicitly allowing it can still open the original URL.

## Details

Suggested bucket: **other**. Confidence: **likely**. The control flow supports this ordering, but the suspension across
deletion has not been reproduced. No unauthorized opening, substituted URL, cross-host effect or loss of user work was
established. The user still has to answer the card; this finding concerns deletion failing to withdraw an earlier
request.

`crates/farhelm-helm/src/approvals.rs:333–381` awaits the host registry and requesting session's title before
registering the approval. Registration at 387–417 checks only a temporary deletion marker. Deletion creates that marker
and denies existing cards at `crates/farhelm-helm/src/sessions.rs:3075–3081`; the marker is removed when deletion
finishes (`approvals.rs:242–251`). A request suspended before registration can therefore resume after the whole deletion
and insert a new card. A missing session title is accepted (`approvals.rs:497–509`), and the answer wait checks host
connection liveness rather than session existence (`430–440`).

The supervisor does not prevent this schedule. `OpenUrl` is non-mutating (`crates/farhelm-proto/src/lib.rs:1975–1977`),
so the relay takes no session-deletion fence (`crates/farhelm-supervisor/src/service/handlers.rs:3193–3240`). Deleting
one session leaves its host connection alive; the terminal detach notification is not connection-wide cancellation.
After a raw Allow answer, `crates/farhelm-helm/src/browser.rs:112–134` checks only the original host connection before
invoking the native opener. Tunnel answers have additional session checks and are not claimed affected. The GUI lists
these cards without checking whether the session still exists (`crates/farhelm-ui/src/approvals.rs:95–133`), so
refreshing does not remove them.

Possible cover: `SPEC.md:2594–2597` explicitly permits an approval after the asking CLI dies, and
`SPEC_impl.md:5534–5546` accepts unfenced link requests and slow resolution. Neither explicitly settles deletion of the
whole requesting session before card registration. `SPEC_impl.md:5605–5607` instead explains deletion denial as
preventing a card from acting for a session going away. This ambiguity remains for triage. No matching Planned item,
BUGS entry, existing queue item or ledger decision was found. The rare self-correcting-glitch filter does not clearly
cover a server-side approval that survives refresh and can still be acted on.

A focused reproduction should pause an admitted OpenUrl request before registration, finish deleting its session while
keeping the host connection alive, then release the request. Inspect the approval listing and use an inert native-opener
spy to check the effect of an explicit Allow. Any fix must preserve the accepted behavior after the CLI alone exits and
the deliberate lack of a long-held session-change fence for link requests.

Provenance: persistent-review A08-F3, corroborated by A05-F1, A14-f3 and A15-withdrawal A15-F1. These are one underlying
defect. A09-002, recorded separately as an expiry-related false delivery acknowledgement, is a different race.
