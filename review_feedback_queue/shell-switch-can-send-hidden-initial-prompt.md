# A shell-to-agent launch can send an initial prompt absent from the visible draft

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Entering an agent choice from a shell draft can send an initial prompt that was hidden on the shell draft.

## Details

**high**. Confirmed by source and contract inspection.

Confirmed by source. No runtime or vendor-transport observation. Draft.rs:320–328 retains the prompt while unsupported
and :726–755 has no prompt-owner check. View.rs:1860–1887 draws the field only for a supported agent and describes it as
that agent's first message, while :1721–1728 omits the whole choices summary on a shell. View/choices.rs:239–246 and
components/radio_group.rs:429–438 apply an activated chip. Components/mod.rs:471–488 then calls Launch in the same
event. Request.rs:509–515 reactivates the nonempty prompt for the newly chosen compatible program and :609–624
serializes it. Farhelm-proto/src/launch.rs:97–110 lists multiple prompt-capable agents, and :113–132 and supervisor
service/core.rs:14489 pass the actual message as program arguments. A supported ordinary guarded start has no
prompt-specific question under SPEC.md:1455–1469. This is payload text, distinct from f18's stale conversation locator,
although both use the same immediate-launch control. SPEC.md:1144–1152 and SPEC_impl.md:8848–8854 accept retaining the
draft and omitting it for unsupported selections. SPEC.md:1394–1396 requires the nonempty prompt's line count on the
agent launch summary. The defect is the unseen activation at submission, without proposing that draft retention be
removed. SPEC_impl.md:8863–8864 separately says a template switch to an unsupported agent drops the dormant prompt, so
the demonstrated route is the hand switch. Estimated high for passing an unintended message to a newly selected process.
The source does not establish a particular provider's later transmission or other higher consequence. Possible_cover:
known draft retention and SPEC.md:1123–1125’s choose-then-launch behavior, neither of which explicitly says an unseen
reactivated prompt is authorized. No later affected source change through pinned
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-launcher/f19`.
