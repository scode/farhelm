# Terminal selection highlights unused wrap padding

Reviewed commit: ecbe3fb7e1448f0a40a2e36fafdfc2f4ef1d6ce6

## TLDR

Selecting text across a wrapped wide character can highlight the unused cell at the end of the earlier row. That cell is
absent from the copied text.

## Details

`crates/gui/farhelm-gui/src/terminal.rs:809` draws each selected range through the row width. The grid model skips the
unused padding during text extraction, so selection and copy differ at that boundary.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`. Findings: `terminal-selection-links/r4`.
