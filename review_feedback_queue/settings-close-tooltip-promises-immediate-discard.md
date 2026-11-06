# Settings close tooltip promises immediate discard

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

Settings' Close tooltip says edits will be discarded even though unsaved recipe changes open a guarded leave question.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/settings.rs:791-792 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g056/g056-f7, event 15):

Reviewed full hash: 68790603037bb51ccd8655e4a948c7ffb747760a. TLDR: Settings tells users Close discards unsaved changes
even when their recipe changes will instead trigger a Save/Discard/Keep editing question. The assigned change routes
Escape/header/footer through request_close (settings.rs:302-312,341,351,792) so a dirty recipe editor can hold closing.
But both header dismiss_tooltip (:350) and footer data-tooltip (:791) retain the unconditional text 'Close settings;
unsaved fields are discarded'. SPEC.md Topology's Tensorlake recipes paragraph at this revision (:302-305) requires
asking on Settings close and completing close after Save/Discard. That question is intentional and implemented by
DialogCloseGuard/hold_close, so the leftover help now describes the wrong result for this supported dirty-recipe state.
This is a wording/convenience defect only; no loss of recipe changes or other user work is established. Suggested
bucket: other. Confidence: confirmed source-level mismatch between tooltip and supported behavior; no runtime experiment
performed. Fix the two tooltips to mention that unsaved recipe changes ask first, or use neutral close wording, while
preserving the different discard behavior of other explicitly saved fields. Persistence checked at pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b settings.rs:308-314,353-354,794-795: same guard and inaccurate text.
proposed_drop: false. possible_cover: unknown; this is a NEW local discovery, distinct from g056-l3's uncertain payload
timing and g058-l4's known stale continuation defect. Parent handles coverage. Caveat: impact is misleading hover help,
not confirmed data loss or failure of the close guard.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g056/g056-f7`; supporting
records `g064/f11`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
