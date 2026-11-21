# Stop's answer help describes a running shell when only tabs remain

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

When only extra tabs remain, the Stop dialog’s help still says it will stop the shell.

## Details

**other**. Confirmed by source and contract inspection.

Source-confirmed. Stop.rs:96–105 says “Close N terminal tabs?” for an ended main program; :139–165 uses only
program_word and always says “Stop this session's {program} and close its terminal tabs” or “Cancel: keep the {program}
and terminal tabs running”. SPEC.md:1997–2003 says tabs can remain on independent main-terminal exit and requires a
tabs-only question then. Stop.rs:70–74 freezes refuse_if_agent_running, which checks the original main process.
Clarification from shell-runtime/f14 and shell-contract/f18,f19: that guard can still allow the sweep of identifiable
surviving main-shell work. The wording discrepancy here does not cause or authorize that cleanup. Keep the real
process-loss route with those records, including Stop and suspend's inline alternative. Estimated other for
contradictory help alone. Possible_cover: none. Shell-specific wording was added here to an inherited status-insensitive
help path. No later correction through pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-presentation/f9`.
