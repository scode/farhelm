# AgentKind::restart_readiness returns None for AgentKind::Custom, breaking the documented contract and misreporting Restart with as "Custom sessions can't be resumed"

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

For a registered custom agent session created without session resumption enabled, the disabled Restart with menu item
tells the user "Custom sessions can't be resumed" instead of explaining that this launch has no stored resume command
and pointing them to Clone or Replace with.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/lib.rs:1750`.

`crates/farhelm-proto/src/lib.rs:1722-1752` documents `AgentKind::restart_readiness` as returning `None` "for kinds with
no conversation reporting", but commit `7164b111` added `AgentKind::Custom | AgentKind::Generic => None` (line 1750)
because a custom agent's reporting moment is caller-determined. Because `None` now conflates "no conversation reporting"
with "unknown reporting moment", `crates/gui/farhelm-gui/src/launcher/entry.rs:369` (`restart_with_reason`) checks
`if selection.harness.agent_kind().restart_readiness().is_none()` before checking
`else if offer == RestartOffer::NoResumeCommand` (line 371). For any `SessionLaunch::Agent` with `LaunchHarness::Custom`
and `resume: None` (a custom agent registered with `supports_resumption = false`, regardless of `supports_reporting`),
`entry.rs:369-370` short-circuits into `unsupported_resume_reason(LaunchHarness::Custom)`, which formats
`harness_label(LaunchHarness::Custom)` (`"Custom"`, `crates/gui/farhelm-gui/src/launcher/draft.rs:1538`) as
`"Custom sessions can't be resumed"`. That misleads the user into thinking custom agents do not support resumption,
instead of reaching `entry.rs:371-375`
(`"This agent launch has no resume command, so it cannot be restarted; use Clone or Replace with"`).

How to verify: Inspect `crates/farhelm-proto/src/lib.rs:1722-1752` and
`crates/gui/farhelm-gui/src/launcher/entry.rs:363-381`, or evaluate `restart_with_reason` on a `SessionRow` whose
`info.launch` is
`SessionLaunch::Agent { selection: LaunchSelection { harness: LaunchHarness::Custom, custom_agent: Some("acme"), .. }, resume: None, custom_reporting: Some(true), .. }`
and `info.restart_offer == RestartOffer::NoResumeCommand`.

Fix: Separate "supports conversation reporting" from "has a known built-in reporting moment" on `AgentKind` (for
example, an explicit `AgentKind::supports_conversation_reporting(self) -> bool` method or updating `restart_readiness`'s
doc contract and using `offer == RestartOffer::NoConversationReporting` / `supports_conversation_reporting()` in
`crates/gui/farhelm-gui/src/launcher/entry.rs:369`) so `AgentKind::Custom` with `resume: None` reaches the
`NoResumeCommand` explanation rather than claiming custom sessions can't be resumed.

Reviewer metadata (g41a_custom_agents_proto_types/f3): suggested bucket: other — misleading disabled-action explanation
and broken method doc contract; Restart with is genuinely unavailable when `resume` is `None`, so there is no security,
data-loss, or functional-blockage impact.; confidence: confirmed (source analysis).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41a_custom_agents_proto_types/f3`.
