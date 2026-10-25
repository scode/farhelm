# Host Remove's duplicate-answer documentation is too broad

Reviewed commit: 3a359a3b47d752caafc5af4af82b185ac984a6d5

## TLDR

The GUI contract says Host Remove refuses every second answer during its request, but another permanent answer can still
change the future confirmation preference. The helper explicitly preserves that same-burst behavior; the operation
itself remains guarded.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `SPEC_impl.md:792-794`. Locations and revisions in the assessment below identify the reviewed source;
later main is outside this review's pinned range.

Reviewed commit: 3a359a3b47d752caafc5af4af82b185ac984a6d5. TLDR: The new central implementation contract says Host
Remove refuses a second answer during its request, but a second answer can still turn off future host-removal questions.
This is a documentation correctness finding, not a request to change the accepted product behavior. Original shipped
controls hosts/settings_dialog.rs:780-800 pass both rendered-token answers with only rendered disabled props.
hosts.rs:1737-1755 accepts that token and stores skip_host_remove_confirmation before run at 1758; run's actual
synchronous claim at 1583-1597 prevents another removal request only. answer_remove at 4463-4473 checks currency rather
than busy state; its own docs at 4458-4462 explicitly acknowledge the second preference effect. Thus the new
SPEC_impl.md:792-794 guarantee that the first three stay-open prompts refuse a second answer is false for Host Remove.
Pinned tip 463cac1528b5c5e259404975380d321a3fff0412 retains it at SPEC_impl.md:801-803, with the same source behavior at
hosts.rs:1742-1765,4537-4552 and hosts/settings_dialog.rs:783-803. Suggested bucket: other. Confidence: confirmed static
contract contradiction; runtime event frequency not measured. No highest route established: both answers target the
shown host, only one removal starts, the user explicitly selects the permanent option, and forgetting a host leaves its
sessions running. possible_cover: SPEC.md:3439-3442 accepts delivered same-burst actions, explaining the underlying
behavior but not making the new descriptive guarantee true. TODO Planned bucket is empty at 21-25; no exact BUGS,
queue-index, feedback or TRIAGE_OUTCOMES match found by scoped coverage searches.
plans/queue/replace-yolo-cancel-race.md:31-48 repeats this claim while fixing a different Replace/Cancel trigger, so it
does not remove this documentation defect. FILTER's rare-diagnostic rule concerns operation errors/log diagnostics, not
an incorrect implementation specification. proposed_drop: none. Correct the contract to distinguish refusing a duplicate
removal request from refusing every side effect of its answer.

Provenance: `g231/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
