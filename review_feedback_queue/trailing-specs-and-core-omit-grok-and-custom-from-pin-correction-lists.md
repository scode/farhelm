# SPEC.md, SPEC_impl.md, and service/core.rs omit Grok and custom agents from pin-correction notification and version-0 positional lists

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

The product and implementation specs still list only Claude, Codex, Muse, and Antigravity for pin-correction
notifications and omit custom agents from several positional-attribution and version-0 lists, even though Grok and
custom agents now use the same positional process pin and post their own correction notifications.

## Details

**other.** Source inspection. Main reference `SPEC.md:3220`.

Commits 03186e160a8b4a7a4e72137b4287cb750aa3a68e (#4729) and 52f64646cbe4e54313edfa613a6e034e8e65eee3 (#4734)
standardized and updated positional hook attribution lists across SPEC.md, SPEC_impl.md, and supervisor docs, but left
four authoritative lists incomplete at 1b3fe49af86979aebc2719965682a17f8c6366ac:

1. SPEC.md:3220-3222 (Status, session-tracking notifications) lists only "a Claude, Codex, Muse or Antigravity launch
   whose first conversation report came from a nested process of that agent, corrected when a recorded ancestor
   reports." It omits Grok (which posts NotificationKind::GrokPinCorrected in
   crates/farhelm-supervisor/src/service/core/vendor/grok.rs:244-247, already stated at SPEC.md:4162-4163) and custom
   agents (which post NotificationKind::CustomPinCorrected in
   crates/farhelm-supervisor/src/service/core/vendor/positional.rs:71-72, 233-234; custom-agent correction notifications
   are also unmentioned in SPEC.md:4088-4093).
2. SPEC_impl.md:5833-5836 (Session notifications) lists only "a Claude, Codex, Muse or Antigravity process pin
   corrected" and "ClaudePinCorrected, CodexPinCorrected, MusePinCorrected and AntigravityPinCorrected", omitting
   GrokPinCorrected (SPEC_impl.md:4293) and CustomPinCorrected
   (crates/farhelm-supervisor/src/service/notifications.rs:88, 98, 187, 191).
3. SPEC.md:4196-4197 says "Claude, Muse and Antigravity admission require the positional attribution and pin checks
   above... These three use version-0 captures" and SPEC_impl.md:4397 says "Attribution checks apply to Claude, Codex,
   Muse, Grok, OMP and Antigravity", both omitting custom agents (which use positional attribution, ReportPin, and
   ownership version 0 in crates/farhelm-supervisor/src/service/core/vendor/positional.rs:65-73, 232 and
   SPEC_impl.md:8938-8939).
4. crates/farhelm-supervisor/src/service/core.rs:15685 (three lines above the doc lines added by 03186e16) says "Claude,
   Muse and Antigravity use a process pin with ownership version 0;", omitting AgentKind::Custom dispatched right below
   at core.rs:15731-15740.

How to verify: Compare SPEC.md:3220-3222, 4088-4093, 4196-4197; SPEC_impl.md:4397, 5833-5836; and
crates/farhelm-supervisor/src/service/core.rs:15685 against
crates/farhelm-supervisor/src/service/notifications.rs:84-98, vendor/grok.rs:244-247, vendor/positional.rs:65-73, and
core.rs:15700-15748.

Fix: Include Grok and custom agents in the pin-correction notification lists in SPEC.md:3220 and SPEC_impl.md:5833-5836
(and note custom agents' correction notification in SPEC.md:4088-4093), and include custom agents in SPEC.md:4196-4197,
SPEC_impl.md:4397, and crates/farhelm-supervisor/src/service/core.rs:15685.

Reviewer metadata (g55_positional_hooks_shared/f1): suggested bucket: other (specification and code-comment drift; no
security, data-loss, or runtime UX degradation, and nothing here misleads a reader into a security or data-loss
mistake).; confidence: confirmed; possible cover: none.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`, `SPEC.md:3220` (g56_positional_hooks_grok/f1):

- `crates/farhelm-supervisor/src/service/notifications.rs:88,123,144,167,187,216-218` and
  `crates/farhelm-supervisor/src/service/core/vendor/grok.rs:244-247` add and emit `NotificationKind::GrokPinCorrected`
  (`grok_pin_corrected`), and `SPEC.md:4162-4163` and `SPEC_impl.md:4292-4293` document it.
- However, the authoritative notification inventories in `SPEC.md:3220-3223` (under `### Status`) and
  `SPEC_impl.md:5833-5837` (under `session_notifications`) were not updated by
  `b14f3355cb2e513220e8251112c8316670aad9e7` (nor by follow-up commits `03186e16` / `52f64646` up to
  `1b3fe49af86979aebc2719965682a17f8c6366ac`): both still enumerate only Claude, Codex, Muse, and Antigravity
  (`ClaudePinCorrected`, `CodexPinCorrected`, `MusePinCorrected`, `AntigravityPinCorrected`), omitting Grok
  (`GrokPinCorrected`) (and note that `CustomPinCorrected` from #4723 is also absent there).
- Additionally, `SPEC.md:3222-3223` states that pin-correction text should not claim no further ancestor can correct it,
  whereas `GROK_PIN_CORRECTED_TEXT` (`crates/farhelm-supervisor/src/service/notifications.rs:216-218`) says "reported by
  a child Grok process... reported by the Grok process that started it" (matching Claude/Muse/Antigravity's older
  phrasing rather than `CODEX_PIN_CORRECTED_TEXT`'s updated "another process... one of that process's ancestors"
  phrasing in #4728).
- Verification: inspect `SPEC.md:3220-3223`, `SPEC_impl.md:5833-5837`, and
  `crates/farhelm-supervisor/src/service/notifications.rs:84-98,215-218` at `1b3fe49af86979aebc2719965682a17f8c6366ac`.
- Fix: add Grok to `SPEC.md:3220` and `GrokPinCorrected` to `SPEC_impl.md:5833-5836` (and align
  `GROK_PIN_CORRECTED_TEXT` if desired).

Reviewer metadata (g56_positional_hooks_grok/f1): suggested bucket: other (documentation/specification inconsistency
only; no security, data-loss, or UX degradation impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g55_positional_hooks_shared/f1`,
`g56_positional_hooks_grok/f1`.
