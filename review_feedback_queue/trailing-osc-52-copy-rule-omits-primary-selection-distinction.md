# The OSC 52 copy rule omits the primary-selection distinction

Reviewed commit: dc4f53c28970da126c4c6f481f9061957ee87e5a

## TLDR

The wording can lead readers to expect a primary-selection write to replace the system clipboard.

## Details

**other.** Source inspection. Main reference `SPEC.md:3464`.

SPEC.md 3464–3466 now says an OSC 52 WRITE copies to the clipboard without qualifying the target. The same bullet at
3477 distinguishes primary selection for drag-notice suppression. Protocol grid.rs 228–240 and 1156–1157 distinguish
clipboard writes from primary-selection completion. Grid model lib.rs 359–366 returns text for
ClipboardTarget::Clipboard only. Attached/tests.rs 7773–7802 explicitly expects a primary-selection write to leave the
clipboard alone while suppressing the notice. Qualify the copy sentence with the target and leave the notice rule as
written. Bucket other for inaccurate documentation. Confidence confirmed by source and existing assertions. Possible
coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l9`.
