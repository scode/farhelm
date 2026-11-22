# Retired YOLO flag refusal overstates when Farhelm asks

Reviewed commit: 38b9760554b3c6ee72e3b6c949c6459ed04e3101

## TLDR

The retired --confirm-yolo flag tells agents that every launch requires the user to answer even though host command
permissions can waive ordinary cards.

## Details

**other.** Source inspection. Main reference `crates/farhelm/src/main.rs:820`.

refuse_confirm_yolo in crates/farhelm/src/main.rs around line 820 uses “before any agent launch.” SPEC.md:4523–4535
describes ordinary approval and its command-permission exceptions, and 4541–4546 explicitly distinguishes compulsory
wrapper approval from hosts that run ordinary agent commands without asking. Keep the obsolete-flag and YOLO-guard
explanation and remove the blanket approval claim. Bucket: other (misleading guidance, no change to enforcement).
Confidence: confirmed by source and contract. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `notices/f1`.
