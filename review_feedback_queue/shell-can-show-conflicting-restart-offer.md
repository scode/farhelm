# Contradictory supervisor metadata can offer conversation Restart for a shell

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Conflicting host metadata can make the app offer conversation Restart on a Bash or Zsh session.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed as a conditional display and dispatch path, estimated other. The helm's aggregate.rs:207–235 preserves the
supervisor's SessionInfo when forming its row. GUI restart.rs:49–69 trusts the independent RestartOffer for offered
state and the accessible label; :104–110 only checks shell identity in the NoConversationReporting branch, so Resume
still claims a conversation. Session_pane/header.rs:221,368,819 uses that offer and restart.rs:231–268 can submit the
drawn session's action. Launcher/entry.rs:333–342 rejects Restart with based on shell launch identity, so that separate
route is unavailable. The shipped Generic supervisor produces NoConversationReporting, as independently checked by
shell-runtime/f8 and candidate f7. With conflicting peer metadata, the outbound request is resolved and sent to the
displayed session's own host (helm sessions.rs:3375–3413,3466–3480), and the shipped supervisor revalidates before
destructive work (SPEC_impl.md:8465–8477). No route to cross-host effects, secrets, or terminating a shell in the
shipped supervisor is established. SPEC.md:2037–2038 makes both Restart actions unavailable on shells; :5232–5235
requires treating remote supervisor messages as untrusted. Like shell-permissions/f4 it concerns inconsistent peer
observations reaching presentation, but it uses RestartOffer and is independent of the Claude hint. Possible_cover: none
for the erroneous affordance, No later edits to the Restart projection through pinned tip
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-presentation/f14`.
