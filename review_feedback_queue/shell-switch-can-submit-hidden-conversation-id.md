# A hidden conversation selector can follow a shell-to-agent launch

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Entering an agent choice from a shell draft can submit a conversation ID typed earlier for a different agent.

## Details

**high**. Confirmed by source and contract inspection.

Confirmed source behavior, no runtime observation. Draft.rs:726–755 changes agent, wrapper and flags, but does not clear
or bind `conversation`; :834–836 stores it as one draft string. View.rs:1820–1839 draws the selector only when the
current harness offers it. View/choices.rs:239–246 wires a chip's choice and then press, and components/mod.rs:471–488
invokes `apply` then `main` synchronously. Request.rs:520–526 tests only the newly chosen harness capability and whether
the stored string is nonempty, and :609–624 includes it in the new create. The helm validates the string for the newly
chosen harness and uses that harness's named start at launches.rs:862–878. The selector is not matched to the agent it
was entered for. This verifies the wrong request input, not that two vendors recognize the same identifier or that a
particular session would be resumed. SPEC.md:1033–1036 says shells have no conversation; :1135–1143 describes a one-time
selector whose meaning and label belong to the agent; :1123–1125 authorizes applying the selected chip and launching
without a second action, not a prior agent's selector silently acquiring a new meaning. Estimated high for changing a
fresh launch into an unintended named launch or a launch failure without the input being shown. No established data-loss
or security consequence. Possible_cover: neither the explicit choice-button rule nor :1148's stated persistence of the
separate initial prompt specifies cross-harness selector reuse. The retention predates the change. This commit adds a
shell selection that hides the retained value and a direct keyboard route back to an agent. No later launcher edit
through pinned 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-launcher/f18`.
