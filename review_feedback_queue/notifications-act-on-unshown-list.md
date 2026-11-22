# Same rail-notice presentation defect as adjacent IC-04

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Closing Notifications or clearing resolved entries can act on notices the window has drawn but has yet to show.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:3193–3199; SPEC_impl.md:1027–1037, 8389–8395;
crates/gui/farhelm-gui/src/rail_notices.rs:350–382, 411–466, 498–514; IC-04 cites the same route at
f06d1a282401e82da49615640333f53ba1ad2e81

A notification can lose its unread mark, or a resolved notice can be cleared, before the window has presented the list
including it. CC-07 and IC-04 identify the same render-time snapshot, release handler and Clear resolved handler. The
documented dirty-frame key dispatch explains why using the last drawn snapshot is insufficient. Both specs include
off-viewport rows in the last displayed list. SPEC_impl.md also says a later arrival stays unread and limits deletion to
IDs from that list that are still resolved when acted on. That server check does not prove the user saw the list.
Suggested fix: keep the last presented ID snapshot, or hold the action until its frame is presented. The reviewer
suggested other. Confidence: confirmed from source and the documented presentation model. on substance. Possible
coverage: IC-04, consolidate these IDs into one review item.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `contract-corrections/CC-07`, with
`implementation-contracts/IC-04`.
