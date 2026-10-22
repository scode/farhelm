# Duplicate pane facts can remove startup claim protection

Reviewed commit: 2ea2826caa0eec867c4e2bddc01badbac7bbe6c7

## TLDR

Deliberately forging a duplicate pane row through Farhelm's private tmux server can make startup cleanup remove a
checkout that is still being prepared. Directly changing that server is explicitly unsupported. This record preserves an
ambiguity with the separate promise to retain claims after uncertain observations; no ordinary-use trigger has been
established.

## Details

Suggested bucket: conditional **high**, for interruption of active preparation. Confidence: **confirmed conditional
source mechanism; acceptance scope ambiguous**. Runtime frequency is unmeasured. No accidental producer, loss of
existing user-owned work, security-boundary crossing, cross-host effect or lasting installation damage was shown. Those
stronger consequences are not asserted.

`crates/farhelm-supervisor/src/tmux/control_codec.rs:37–40,108–177` includes session names verbatim and recognizes
duplicate valid pane IDs, then removes them from the returned map. A deliberately crafted session name can supply such a
duplicate. `tmux.rs:4151–4187` returns the parsed map successfully. `service/status.rs:145–156` classifies a missing
entry as absent, and startup skips protection for it (`service/core.rs:6905–6947`, especially `6913,6933`). Cleanup then
removes unprotected claims (`service/core.rs:7080–7085`; `spare_checkouts.rs:38–48,541–554`). The recognized duplicate
ambiguity is no longer available to the caller; absence in this map is not necessarily observed pane death.

An ordinary query error takes another path that protects all claims (`service/core.rs:6880–6888`). This finding does not
reopen the independently refuted query-error or pre-clone sampling allegations. The integrated consequence is pinned to
the enabled tip; the original sampler change had not yet enabled spare consumption.

Possible cover: `SPEC.md:3164–3187` accepts deliberate same-account interference, and `3383–3396` explicitly places
direct changes to the private tmux server outside every Farhelm guarantee, including cleanup and recovery. That is
strong potential coverage. The local-trust section also preserves protections specified for particular cases, while
`SPEC_impl.md:2420–2427` specifically requires retaining claims after uncertain liveness observations. An independent
auditor could not unambiguously settle precedence for this recognized parse ambiguity. No exact ledger waiver, Planned,
BUGS or queue match was found. Proposed drop: none approved. Resolve that contract question before adding complexity to
handle deliberate private-server manipulation.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g047/discovery-f1`, independently retained
conditionally by `g059/g047-discovery-f1`. This is an acceptance question with a conditional source counterexample, not
a demonstrated ordinary-use data-loss bug.
