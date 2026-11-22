# Regenerating the link oracle loses the wide-wrap padding case

Reviewed commit: ecbe3fb7e1448f0a40a2e36fafdfc2f4ef1d6ce6

## TLDR

Regenerating the terminal link fixtures can drop the row flag that distinguishes printed spaces from wide-character wrap
padding.

## Details

The wide-wrap case in `crates/gui/farhelm-grid-model/tools/generate-links.ts:161` constructs its row without the padding
marker in the checked-in oracle. A regenerated fixture no longer checks that the link crosses that specific kind of row
boundary.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`. Findings: `terminal-selection-links/r7`.
