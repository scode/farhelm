# Approval views repeat their waiting and answering checks

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Host-add and tunnel approval views independently look up whether a request is waiting or being answered.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

The host-add and tunnel dialogs both query the same approval row and operation key by request ID. Give the approvals
module one helper for each query and have the dialogs use them.

Reviewed locations: `crates/gui/farhelm-gui/src/approvals/host_add.rs:95-110`,
`crates/gui/farhelm-gui/src/approvals/tunnel.rs:173-182` and :194-202. Haiku out-P2.md §1. Suggested bucket **other**.
