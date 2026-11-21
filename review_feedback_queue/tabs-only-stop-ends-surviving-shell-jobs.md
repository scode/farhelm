# A tabs-only Stop can also end jobs left by the main shell

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

A Stop confirmation that only names extra tabs can also end background work left by the main shell.

## Details

**highest**. Confirmed by source and contract inspection.

Confirmed conditional source route. GUI session_list/stop.rs:70–80 and :96–105 builds `refuse_if_agent_running: true`
and the `Close 1 terminal tab?` / `Close N terminal tabs?` consequence from that state, and :139–153 freezes and
displays it. Supervisor service/handlers.rs:855–886 checks the main terminal's live PID, which is absent when its direct
child has exited. After the ended-terminal classification, the same request calls reap_process_tree with the old launch
scope, no PID root, and SweepTarget::AgentOnly (:1107–1130), before closing tabs (:1151–1155). A standard Linux cgroup
can still contain an identifiable background job after the shell exits (scope.rs:6–15, terminal-host/SPEC.md:128–145).
The fallback can find marked survivors subject to the explicit limits in SPEC.md:2169–2179. The handler itself says the
scope outlives the leader as long as it has a descendant. No special platform shell settings or particular job were
observed. The condition is that identifiable main-shell work remained. SPEC.md:1997–2003 generally promises Stop reaps
the tree and explicitly promises that the ended-main-terminal question `asks about the tabs alone`; :2128–2138 limits
destructive consent to the words the user answered. Possible_cover: :2147–2149 describes later cleanup of daemonized
survivors and :1997–1998 describes the overall Stop operation, but neither explicitly authorizes doing it from a
question that only names tabs. Estimated highest for potential loss of running user work beyond the drawn consequence.
Distinct from shell-runtime/f4 (which ends a still-live Bash without an orderly history write), shell-presentation/f9
(contradictory answer help after exit), and shell-presentation/f12 (live UI preference and guard capture). This route
works with a correctly frozen guard. No later change to the Stop handler or prompt in the reviewed range. Source and
documented ownership only. Follow-up source check: the ordinary session menu exposes Stop for an ended, non-busy row
(session_list/menu.rs:1068–1083), and stop.rs:209–227 sends the exact confirmed guard. Although the menu description
mentions ending the shell and its processes in general, the subsequently answered dialog in this state names only
closing tabs. The consent rule binds the dialog, so the earlier generic menu description does not cover the omission.
Terminal_process.rs:15–41 confirms Dead concerns the original terminal child and yields no live PID.
E2e/boot_id_durable_outcome.rs:649–691 tests the retained plain exit annotation for an already-ended session, not
guarding surviving main-shell jobs. The reported supported route is ordinary Linux process containment described in
terminal-host/SPEC.md:128–145 and scope.rs:6–15. No assertion about macOS behavior or exact test timing.

Include the inline Stop and suspend path, which sends the same ended guard and invokes the same cleanup. The inline Stop
and suspend answer uses the same guarded Stop first. Its statement that the other processes resume after the freeze
omits the prior shell work that the Stop can terminate.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f18`,
`shell-contract/f19`, `shell-runtime/f14`.
