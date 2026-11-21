# The status spec omits Replace as recovery for interrupted shells

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

The Status spec omits Replace as a way to recover an interrupted Bash or Zsh session.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed documentation contradiction, bucket other. SPEC.md:2896–2897 says `restart or delete are the ways out`;
:2037–2038 states `Their Restart actions are unavailable, and Replace starts a fresh shell`, and :2264–2267 likewise
prescribes Replace on the recovery card. The implementation follows the shell-specific recovery contract for its own
supervisor: supervisor agent_kind/mod.rs:1909–1912 returns `NoConversationReporting` without an integration. GUI
session_list/restart.rs:104–119 identifies the shell and explains Replace, and session_pane/card.rs:279–281 and :314–321
names the interrupted shell and tells the user to choose Replace. This spec issue covers the ordinary shipped supervisor
result. Shell-presentation/f14 separately owns the misleading Restart presentation under inconsistent remote metadata.
05ae73f23eb2cbb7ab284ca42473d66a771207a9 did not change Status and the contradiction remains at
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Possible_cover: none.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f13`.
