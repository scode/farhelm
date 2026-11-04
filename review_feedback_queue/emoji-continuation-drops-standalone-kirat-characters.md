# Emoji continuation handling drops standalone printable Kirat Rai characters

Reviewed commit: af585f5ba3d90f5826febc24f7954913b0a32205

## TLDR

Printing a standalone Kirat Rai character such as U+16D67, or printing it after an ASCII letter, makes that printable
character disappear from the terminal and copied text. This happens without any emoji or variation selector.

## Details

`vendor/alacritty_terminal/src/term/mod.rs:1473–1474` classifies U+16D67 as V. The live input path at `1519–1523` sends
V/T input to `input_emoji_jamo` and returns before ordinary scalar printing. The helper at `1248–1263` retains input
only after particular stored Jamo/Prepend classes. It rejects the V after an ASCII A, so `A + U+16D67 + B` stores `AB`.
Column-zero input is rejected by the attachment-point check at `1137–1138`.

Existing test premises make the ordinary-printable case explicit: `emoji_selectors_follow_rejected_jamo` at
`term/mod.rs:3930` asserts scalar width `Some(1)` for U+16D67.
`emoji_jamo_drops_illegal_and_baseless_suffixes:4013–4025` pins standalone U+16D67 disappearing on a five-column screen
without emoji or selector. `emoji_jamo_legacy_replay_keeps_scalar_behavior:4029–4045` contrasts that with the old scalar
occupying its own cell.

The stored-text route reaches copy through `crates/farhelm-supervisor/src/grid_output.rs:166–195`,
`crates/farhelm-ui/assets/grid-buffer.js:69–91,126–149,432–444`, `grid-terminal.js:1015–1019,1494–1497,1603–1610`, and
`terminal.js:5325–5327`. A character omitted from terminal storage is omitted from copied text as well.

The reviewed terminal fidelity/copying contract is `SPEC.md`, “Terminal experience,” 2084–2400 and `SPEC_impl.md`,
“Terminal widget,” 1303–1764. The emulator selector contract at `SPEC_impl.md:2572–2613` specifies selector widths and
two edge exceptions, rather than unconditional loss of ordinary printable V/T characters. Preserve ordinary printable
input when contextual attachment rejects it; selector eligibility and snapshot constraints still need to be respected
when choosing the implementation.

## Confidence and priority

Confidence: confirmed source path and explicit existing test premises, without runtime execution. The affected source
remains unchanged through `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`. Legal composed Jamo attachment is not alleged
wrong. Legacy snapshot painting preserves saved layouts and does not repair future live input.

Original reviewer suggested bucket: `other`, for uncommon-text rendering/copy fidelity. Recording bucket: **highest**. A
fresh independent priority consultation could not clearly place deletion from retained terminal text outside loss of
user data, so the queue's ambiguity rule keeps highest. The character deletion is established; file corruption,
credential/process loss and a concrete irreversible useful-work sequence are not. The original `could_be_highest: false`
assessment is preserved as reviewer provenance, not the final priority judgment. `proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

The related policy-overreach findings `g034-l5` and `g043-l6` concern the ignored-format classifier. This V/T branch
deletes printable input outside `emoji_ignored_format` and remains a separate mechanism.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, owner `g043`, ID `g043-f7`, latest `g043/findings.jsonl` physical
line11. The latest record checks the complete no-selector baseless-input tests, legacy comparison and downstream copy
helper.
