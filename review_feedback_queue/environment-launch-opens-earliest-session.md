# An environment launch guesses which session to open

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

If two sessions appear in a new environment before launch finishes, Farhelm can open the other session when their
creation times tie or arrive out of order.

## Details

Haiku pass H-P4-6, checked by gpt-6.2-sol. Suggested queue bucket **other**. Confirmed timestamp heuristic. A supported
sequence admitting a second session in time, then selecting the wrong row, remains unverified.

- Verdict: NOT CRITICAL
- Route checked: `crates/gui/farhelm-gui/src/launcher/launch.rs:793-806,871-910,922-950`,
  `crates/farhelm-helm/src/provisioning/tensorlake_add.rs:331-355,390-400,1073-1137`, `sessions/sandbox.rs:113-119`,
  `sessions.rs:890-910,1853-1869,2241-2264`, `aggregate.rs:225-234`,
  `crates/farhelm-supervisor/src/store.rs:1343-1347,2783-2790`. Old app:
  `24bc0580:crates/farhelm-ui/src/api/sandbox_sessions.rs:104-123`.
- Reason: Each launch registers a fresh session-owned host, and the completion read is scoped to that run's host.
  Freshness supports the normal one-session case, but whole-second wall-clock timestamps can tie or go backward. If
  another session appears before the read, the minimum is not guaranteed to name the launch. Another session needs an
  admitted local or same-environment create, which asks for agent consent by default, and then a tie, reversed clock, or
  absence of the original row. No ordinary shipped ordering causing wrong selection was established. The browser app
  took the exact session from its final response, so the inference is new to GPUI.

Return the exact launched session reference in the completion receipt and open it directly. Test completion with two
rows on the same host and equal creation times.

Possible cover: fresh session-owned hosts normally have one session. `sandbox-create-reply-loses-new-recipe-default.md`
concerns recipe defaults after a different completion path.
