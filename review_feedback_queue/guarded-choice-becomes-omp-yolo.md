# Switching to OMP discards an explicit guarded choice

Reviewed commit: 5837dfd55e4874644ffb479c4824056650c84ec9

## TLDR

Choosing guarded permissions and then switching the launcher to OMP can select YOLO and launch without safety checks on
a host that allows YOLO without asking.

## Details

Suggested bucket: `highest`. Confidence: Confirmed static trace; no runtime reproduction. Source: dedicated
`gpt-6.1-sol` high adversarial review of this commit. Proposed drop: none.

The commit makes Guarded the omitted mode for Claude, Codex, Grok and Muse
(`crates/farhelm-proto/src/launch.rs:203–213`). Harness reconciliation clears any permission equal to the source's
omitted mode (`crates/farhelm-proto/src/launcher.rs:101–104`), including an explicit guarded click. `request_permission`
(`158`) fills OMP's YOLO default. OMP supports Guarded, so the incompatibility explanation is absent
(`crates/farhelm-ui/src/launch_composer.rs:1562–1575`). The selected YOLO mode is visibly painted; this is not a hidden
mismatch between command and UI.

Choose one of those four agents, explicitly choose guarded, then switch to OMP. The launch uses `--approval-mode yolo`
on a permissive host. An ask-first host still requires YOLO confirmation. SPEC.md:548–549 only authorizes clearing
incompatible choices and the previous agent's default YOLO; compatible Guarded is outside that rule.

Restrict default-mode clearing to YOLO, or settle the broader behavior with the maintainer and amend the authoritative
contract.

Possible cover: plans/reports/guarded-permission-mode.report.md:165–177 explicitly records the behavior and reverted
attempted fix under 'For the maintainer to decide'. It does not establish accepted SPEC, Planned, BUGS, queue or
triage-ledger coverage; preserve this context for triage.

Current-main status: persists by inspection at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; no fixing commit found. The
reviewer read the full commit diff and relevant traces; this is not an exhaustive review of unchanged surrounding code.
