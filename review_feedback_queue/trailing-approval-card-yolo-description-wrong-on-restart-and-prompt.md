# ChipKind::Yolo approval-card description claims host YOLO rules apply on Restart and screen-observed Prompt cards where they do not

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

On an agent approval card for a plain Restart of a YOLO session (or a Prompt card whose target session was seen
bypassing checks on its status line), the Permissions row says "Farhelm applies the host's YOLO rules to this launch."
That is inaccurate in both cases: plain Restart is exempt from the host's YOLO launch rule even on hosts that ask before
YOLO launches, and status-line YOLO observation is display-only and never triggers host YOLO rules.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/approvals/model.rs:1065`.

Commit 7b226839 changed `ChipKind::Yolo.description()` in crates/gui/farhelm-gui/src/approvals/model.rs:1065 from
`"YOLO permission bypass"` to `"Farhelm applies the host's YOLO rules to this launch"`. `Chips::of` and
`approvals/card.rs:574` render that description on every card that carries a `Fact::Permission`: (1)
`ApprovalAction::Restart` (approvals/model.rs:545-563, calling `launch_facts(launch, Runs::Resume, &mut facts)` ->
`Chips::of(launch, false, Detail::Launch)` at line 747), even though SPEC.md (Agent-spawned sessions, lines 4498-4499
and 4630-4631) explicitly exempts plain Restart from the host's YOLO launch rule including for YOLO sessions; and (2)
`ApprovalAction::Prompt` (approvals/model.rs:516-537, calling `Chips::of(launch, *screen_yolo, Detail::Plain)` at line
526), where SPEC.md (line 4445) states the YOLO launch rule does not apply and (lines 2879-2880) status-line YOLO
observation (`screen_yolo`) is display-only and never consulted by host YOLO guards — yet when `screen_yolo == true`
escalates a Guarded session or an asserted-not-YOLO command to `Shield::Yolo` -> `ChipKind::Yolo`, the card claims
Farhelm applies the host's YOLO rules to this launch.

How to verify: Inspect approvals/model.rs:526, 560, 747, 1024-1028, 1063-1065 and approvals/card.rs:574, or construct
`Card::of` for an `ApprovalAction::Restart` of a YOLO session or an `ApprovalAction::Prompt` with `screen_yolo: true` on
a Guarded launch.

Fix: Word `ChipKind::Yolo.description()` around permission bypass treatment rather than claiming host launch-gate
enforcement (for example, matching `permission/words.rs:104`: "Treated as YOLO — bypasses approval and sandbox checks",
or distinguishing observed-on-screen bypass on Prompt cards).

Reviewer metadata (g20_yolo_unset_gui_launcher/f5): suggested bucket: other — misleading text on Restart and (currently
dormant) Prompt approval cards; the warning chip still draws in amber as YOLO and enforcement/approval gates are
unaffected, so there is no security bypass or data-loss risk.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g20_yolo_unset_gui_launcher/f5`.
