# Add Host reuses an earlier sandbox confirmation setting

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

If the confirmation setting changes while Add Host stays open, the form can skip the question before starting a billed
Tensorlake sandbox.

## Details

Haiku pass H-P3-3, checked by gpt-6.2-sol. Suggested queue bucket **other**. Confirmed stale snapshot and lack of a
server recheck on this path. No supported way to change this setting behind an eligible open form was established. A
shipped route that bypasses required authorization would change the priority.

- Verdict: NOT CRITICAL
- Route checked: `crates/gui/farhelm-gui/src/hosts/add/tensorlake.rs:225-247,394-434`, `hosts/setup.rs:421-430,505-530`,
  `crates/farhelm-helm/src/provisioning/http.rs:208-217`, `provisioning/service.rs:1258-1273`,
  `provisioning/tensorlake_add.rs:894-1008`, `sessions/sandbox.rs:89-103`, `ui_channel.rs:960-968,980-1025`.
- Reason: The scanner's assumed helm recheck is only for launching a sandbox session. Add Host sends the same provision
  token after explicit confirmation or an automatic skip, so its handler cannot distinguish them or recheck revocation.
  SPEC.md:381-394 reserves the skip for the Tensorlake preference. No ordinary shipped way to turn that setting off
  behind an already keyed Add Host form was established. The GUI holds a single seat and serializes commands, the normal
  Settings link inside Add Host is for the no-key state and refreshes the account, and there is no corresponding agent
  or CLI setting command. The stale snapshot is real, but the bypass needs a concurrent setting write outside that
  supported path. The old app also used a cached account
  (`24bc0580:crates/farhelm-ui/src/tensorlake/add_host.rs:184-201`).

Read the current Tensorlake account preference before skipping, or send explicit versus skipped consent on distinct
requests and check the current setting at admission.

Possible cover: SPEC.md:381–394 permits skipping by the Tensorlake setting. The single-seat behavior narrows the
trigger. Neither passage explicitly accepts using the earlier setting.
