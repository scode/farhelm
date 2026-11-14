# The deleted raw-input test leaves cursor/control-sequence delivery without its end-to-end oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The deleted raw-input test leaves cursor/control-sequence delivery without its end-to-end oracle

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g141/c4`.

### g141/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513

## TLDR

The removed raw receiver test proved that a complete cursor/control sequence reached the program unchanged and in order.
The remaining tests preserve parts of that proof, but do not replace the full raw-sequence oracle; the source explicitly
records that gap.

## Details

This completes synthesis of original c4, narrowing its initial no-control-coverage wording after reading actual
replacements. The assigned deletion removes input_bytes_survive_verbatim_through_hexecho (old side :3586) and
printable_and_binary_input_preserve_bytes_across_commands (:3654), at new deletion boundary :2913. The first sent a,
DEL, b, ESC+[A, ETX and a trailing z through the real attachment, then reconstructed the raw receiver's bytes and
required [61,7f,62,1b,5b,41,03,7a]. The final acknowledgment excluded a truncated or reordered prefix. An equivalent
host/grid test can retain this valid-UTF-8 control sequence; tmux quoting and arbitrary invalid-UTF-8 payload checks are
obsolete. At this revision harness.rs:397-438 maps such send_keys data into GridInput::Text on grid attachments.
grid_input/tests.rs:254-261 validates the isolated text encoder, and service/grid_stream/tests.rs:1014-1023 validates
a/hi/Enter against a scripted host, so neither substitutes for the shipped connection-to-real-PTY boundary.
e2e/tests/terminal-flood.spec.ts:165-175 deliberately excludes ArrowUp because ECHOCTL makes a real ESC and literal
caret text visually identical, and explicitly says: "The Rust e2e test that covered it end to end ran on tmux terminals
and is gone, so that end-to-end check is not covered any more; the grid input encoder's key table still pins the bytes
each key sends." Checked retained raw oracles rather than assuming absence: terminal-keys.spec.ts:215-442 does prove
real ESC/CR and printable sentinels for Shift+Enter and related chords (including grid), but no ArrowUp/CSI sequence,
DEL or ETX; its header explicitly limits its byte-exactness to its sent controls. The ArrowUp hits in
terminal-multihost.spec.ts:1309,1348,1444 navigate host menus, not a terminal program. prompt_running_agent.rs's
recorder delivery proofs are ignored (:463-479), and their live viewer probes are printable strings (:249,383,403,420);
they do not replace the deleted sequence. Therefore the retained finding is a specific lost raw-sequence test boundary,
not a claim that no control byte is tested end to end or that the product is known broken. Suggested bucket: other
(test-only maintainer coverage). Confidence: confirmed by pinned source and the explicit missing-check note; no
execution. proposed_drop: false. Could-be-highest: no established security/loss-of-work route. possible_cover: none; no
excluded coverage or queue searched. Original consequence/caveats retained: old terminals still remain supported at this
intermediate revision even though all fresh launches are hosts; raw transcript uniqueness, tmux argument escaping and
recycled pane ids need different host oracles and are not demanded here. Large printable input is covered by
terminal-flood.spec.ts:85-152, whose >1 MiB real receiver checks count/SHA-256 and later live input. Zero-size handling
is tested at the host boundary by hosts/terminal/tests.rs:2530-2581 (invalid zero-column resize), while
session_lifecycle.rs:2851-2870 retains create clamping; no additional host-attach clamping defect is asserted because
the old tmux clamp is not the host's invalid-size contract. Bracketed paste is encoded at the supervisor, not restored
as old browser byte escapes. Positive live alternate-screen reattach is separately retained under l5, rather than being
dismissed with the obsolete escape ordering. No runtime test, build or reproduction was run.

Discovery turn 3, through-tip repair check: compared later changes from 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513 through
inclusive 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. Read the complete later session_lifecycle diff in three bounded
ranges: it deletes the now-unused hex_tokens helper/test and does not add a raw sequence replacement. Read the complete
terminal-keys later diff: it keeps the same Shift+Enter/Enter/control-shift-enter scenarios while adapting their
wiring/comments to grid; no ArrowUp/DEL/ETX raw sequence is added. terminal-flood later diff keeps its explicit missing
raw end-to-end check note, with wording updated from exists to existed; its >1 MiB digest test remains the
printable-paste oracle. prompt_running_agent and grid_stream/tests later diffs change only comments for these candidate
replacements, not control-sequence assertions. Pinned-tip targeted searches across Rust e2e, browser terminal tests and
grid-stream tests still find only the known raw Shift+Enter proof, ignored prompt recorder, cooked backspace/Ctrl+C case
and host-menu ArrowUp uses. No direct replacement or repair found through the pinned tip. Historical attribution,
test-only consequence, existing partial coverage, confidence and proposed_drop:false remain unchanged. This is a
supplemental source check of the known c4 finding, not a new finding or runtime observation.

Returned proposal (claims-089, event 13): **retain-verified**, suggested bucket **other**.

The removed raw receiver pinned ordered DEL/CSI/ETX delivery. Remaining real-PTY tests pin ESC/CR for other chords and
cooked erase/interrupt behavior, but the source explicitly records loss of the raw cursor-sequence end-to-end oracle.

Original stronger consequence: Initial broad candidate treated all retired tmux tests as nontransferable and proposed
dropping coverage concerns; that rationale was withdrawn. A later claim said control-byte delivery had no end-to-end
oracle; it was narrowed after finding ESC/CR raw chord tests. Preserve the final specific DEL/CSI/ETX sequence gap,
existing partial coverage, and absence of runtime evidence. Bystander-process protection appeared in the original broad
dropped set; raw transcript uniqueness, tmux parser quoting and arbitrary binary input are obsolete for new grids.
Positive alternate-screen reattach was referred to separate l5, not resolved in this batch.

Unresolved premise or limitation: No missing source premise for the narrowed raw-sequence oracle loss. No current
product byte mangling or harmful user-work route has been demonstrated. Earlier broad bystander/alternate-screen
coverage dismissals are not certified by this narrow raw-input check.

Proposed basis: Record the narrowed raw-receiver test gap as other. Before disposing of the earlier broad
bystander-process-protection omission, parent should include that named historical dismissal in the independent drop
register; this consultation does not approve that drop.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g141 record](records/review-through-00de0b6c/groups/g141.json). Record ID: `c4`.
