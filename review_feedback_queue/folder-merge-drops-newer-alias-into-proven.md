# Folder merge drops a newer alias into a proven row without transferring it

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Browsing a folder can permanently erase a folder suggestion's newer spelling: the suggestion becomes unfindable by the
name the user most recently used, even though browsing is supposed to be read-only.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: possible (single-lens, presentation-level loss). The
trigger is ordinary use and existing tests cover only unproven-canonical merge and same-key immunity.

`refine_folder_history`'s merge `UPDATE` (store.rs:4949-4974) moves a newer alias observation's presentation onto the
canonical row only when `canonical.canonical_proven = 0` (store.rs:4966), but the alias `DELETE` (store.rs:4975-4988)
has NO such guard on its canonical `EXISTS` subquery. Proven canonical + newer unproven alias with the browsed display
spelling → merge skipped, alias deleted, newer spelling/recency/ordering lost; the survivor keeps older presentation and
sort position. This contradicts the merge's own rule ("its display spelling always belongs to the newest launch users
can still choose", store.rs:4946-4948) and the upsert path's pinned behavior
(`older_proven_create_promotes_a_newer_legacy_folder`: a newer legacy observation takes over a proven row's spelling
while the proof flag is retained). The trigger is ordinary use (mixed legacy/proven creates, then browsing a directory
linking them); the effect is user-visible (a suggestion unfindable by its newer spelling — a read-only browse
permanently removes it). The merge never touches `canonical_proven`, so transferring presentation would not revoke
proof.

Suggested fix: narrow the `DELETE` to aliases no newer than the survivor, or extend the merge to transfer presentation
onto proven rows leaving key + `canonical_proven` untouched; add a newer-alias-into-proven-canonical test.
