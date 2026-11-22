# Agent create drops the approved target host connection check across post-approval validation

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When an agent asks to create a session on a named host and the launch is not one whose folder is pre-checked for
workspace trust (for example, any Command launch, any launch with workspace trust off, or any Antigravity launch), an
approved YOLO, Command, Custom-flags, or agent-wrapper launch re-checks the target host's connection before its final
database check, then discards that connection check before actually sending the launch. If the target host is retargeted
or reconnects during that post-approval check, the launch can be sent to the replacement connection instead of being
refused.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/agent_requests.rs:2703`.

Source trace at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`:

1. In `create_for_agent` (`crates/farhelm-helm/src/agent_requests.rs:2071-2102`), `composition_incarnation` starts as
   `None` and is populated only inside `if launch.pins_approval_cwd()` (lines 2092-2099). Every Command launch,
   non-trust harness launch, trust-off Claude/Codex/Muse/Pi launch, and Antigravity launch (even with trust on, due to
   `l1`) leaves `composition_incarnation = None`. (Note: commit `6f9918a8` narrowed this from all trust-capable launches
   to `if launch.pins_approval_cwd()`, and `crates/farhelm-helm/src/workspace_trust.rs:50-52` comments that a launch
   without trust does not depend on that connection surviving the wait for Allow, even though `approve_launch:2649,2703`
   already enforces that during the wait.)
2. `approve_launch` (`crates/farhelm-helm/src/agent_requests.rs:2649-2720`) captures
   `target_before = state.manager.status(host).map(|status| status.incarnation)` before `approved(...)`, verifies
   `state.manager.status(host).map(|status| status.incarnation) != target_before` at line 2703, and then awaits
   `crate::yolo_guard::check_agent_approved(state, host, &launch, agent_wrapper_supplied, wrapper_card).await?` at lines
   2711-2718. For every non-vouched or agent-wrapper launch (YOLO, Command, Custom permissions, or agent-supplied
   wrapper), `check_agent_wrapper` (`crates/farhelm-helm/src/yolo_guard.rs:138-146`) awaits
   `state.store.list_hosts().await?`, yielding across the store actor.
3. `approve_launch` returns `Ok(wrapper_card)` and drops `target_before`.
4. `dispatch_agent_create` (`crates/farhelm-helm/src/agent_requests.rs:2953-2960`) then calls
   `crate::sessions::host_client(state, resolved.host)?` and checks
   `crate::precondition::incarnation_holds(&claim, dispatch.composition_incarnation)?`. Because
   `dispatch.composition_incarnation` is `None`, `incarnation_holds` (`crates/farhelm-helm/src/precondition.rs:106-109`)
   returns `Ok(())` unconditionally. When the host was named by `--host` (not a template), `template_host_moved` is also
   a no-op (`resolved.template_identity` is `None`), so `do_create_session` dispatches to the successor connection,
   violating `SPEC_impl.md:9449` ('A launch is also refused if the target host's connection changed while the card
   waited, since the approval was for the machine the card named'). By contrast, `clone_for_agent`
   (`agent_requests.rs:3926,3977-3978`) and `create_checkout_for_agent` (`agent_requests.rs:2222,2390`) keep the pinned
   `claim.incarnation` all the way through dispatch.
5. Caveat: the race window is narrow (the duration of `state.store.list_hosts().await` inside `check_agent_approved`
   between `agent_requests.rs:2703` and `2953`, plus any yield in `create_for_agent:2144-2145`), and only affects
   `--host`-selected creates where `!launch.pins_approval_cwd()` and `!vouched || wrapper_needs_approval`.
6. Fix direction: Have `approve_launch` return the verified `target_before` incarnation alongside `wrapper_approved` (or
   verify `target_before` again at dispatch via `composition_incarnation.or(approved_incarnation)` passed into
   `AgentCreateDispatch`), so `dispatch_agent_create:2954` always holds the dispatch connection to the incarnation that
   was approved. Also correct the misleading comment in `crates/farhelm-helm/src/workspace_trust.rs:50-52` (and the
   obsolete pre-approval comment in `crates/farhelm-helm/src/agent_requests.rs:1937-1946`).

Reviewer metadata (g14-trust-codex-muse-pi-helm/f2): suggested bucket: highest (could be in highest: yes — narrow race
across approval-to-dispatch boundary allowing an approved YOLO, Command, Custom, or agent-wrapper launch on a
--host-selected target to dispatch to a replacement host connection); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g14-trust-codex-muse-pi-helm/f2`.
