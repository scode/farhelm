# Conflicting shell status can show the Claude trust hint

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Conflicting status from a remote host can show a Claude trust hint on a shell session.

## Details

**other**. Confirmed by source and contract inspection.

A managed-checkout shell can show a Claude trust hint when a connected host's status contradicts its recorded launch.
Source-confirmed display path for contradictory remote data: farhelm-proto/src/lib.rs:939-958 gives the two trust
observations independent boolean fields. Farhelm-helm/src/aggregate.rs:207-235 passes SessionInfo through.
Settings/agent_trust.rs:239-247 trusts the Claude observation before the new Bash/Zsh exclusion at :254-267.
Session_pane/trust_band.rs:114-118 and :342-369 accept a fresh managed-checkout row and draw the Claude copy when its
usual preference gates are met. The shipped supervisor does not generate this combination: Generic selects GenericReader
with the default false Claude result (screen_reader.rs:334-343, :383-398), Bash/Zsh also explicitly return false for
neutral trust at :99-120, and ticker.rs:465-478 samples both. SPEC.md:1704-1712 and :1726-1731 limit the hint to the
displayed agent's current trust prompt; :5232-5235 treats remote supervisor messages as untrusted. Consequence
established is a misleading band and settings shortcut only. Opening that shortcut does not turn on trust
(trust_band.rs:434-454), and the off-by-default switch still requires a separate explicit action with caution
(SPEC.md:1688-1702). Bucket other. The possible higher consequence mentioned in the lead has no route through this UI.
Possible_cover: none. No changes to the trust files or screen reader in the rest of the pinned range through
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f4`.
