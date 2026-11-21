# Template editor says custom approvals are absent while retaining them

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

The template editor says a shell has no approvals while retaining incompatible approvals that block saving.

## Details

**other**. Confirmed by source and contract inspection.

Source-confirmed: rows.rs:291–300 sets only the agent. Editor.rs:450–462 syncs the draft. Fields.rs:333–336 offers no
modes. Rows.rs:499–523 returns before rendering the stored words. Fields.rs:211–247 and :435–447 refuse Save.
Rows.rs:199–223 retain removal and refusal. `templates/summary.rs:92–101, 181–183` also labels a stored shell approval
None. Switching to a different capable agent can restore the choice, but the editor then shows the words and the launch
is still Custom. No hidden executable permission or highest consequence is established in this editor. SPEC.md:1238–1245
says `Incompatible fields stay visible and prevent saving until corrected or removed`. SPEC_impl.md:8872–8879
intentionally retains incompatible draft fields, without requiring the stored choice be described as absent. Synthesis
scope: shell-permissions/f9 traces the same `permission_label` into a separate approval interface. Confirmed at pinned
approvals/model.rs:801–816 that a card uses this label and independently prints stored flag words. This revises the
interface boundary only: the local draft demonstrably hides the mode and words, while a card can suppress the mode but
does not suppress words. This record does not adjudicate which incompatible records can reach an agent card. Defer that
to shell-permissions/f9. SPEC.md:4331–4335 and :4353–4354 give the card its separate whole-record and stored-label
contract. Estimated other. Possible_cover: partial preservation by editor refusal. Neighbor f9 overlaps only in the
shared label. No later template editor change at tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. No runtime observation.
The synthesis result in shell-permissions/f9 now establishes the separate shipped CLI write/delete route through helm
validation and the card. It also confirms that applying a concrete permission to a shell is refused. The card result is
a sibling presentation defect, while this editor result is the draft and list defect. Correcting the shared label could
address both list/card surfaces, while the editor control needs its own treatment.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-launcher/f7`.
