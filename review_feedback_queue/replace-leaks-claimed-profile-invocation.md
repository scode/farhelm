# Replace sends a claimed profile's full command line to the remote host

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A compromised remote machine can obtain any saved profile's full command line, including embedded keys, the first time
you press Replace on one of its sessions.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F27 / SEC-REPLACE-LEAKS-PROFILE-INVOCATION`, tagged **possible**. Anchors and title: `sessions.rs:3088`,
`sessions.rs:2038-2047` — Replace launches whichever catalog profile a remote host claims, sending that profile's full
command line to the host

A profile's **invocation** (the full command line, which can embed API keys) and its **resume template** are sent to the
target supervisor whenever a session is created from that profile. In a plain Replace (`sessions.rs:3088`),
`mode_from_source` trusts the source row's `source_profile.id` as the supervisor reported it. It looks the id up in the
helm-wide catalog (`sessions.rs:2038-2047`) and creates the replacement from that profile, sending its invocation and
resume template to the session's host. Nothing checks that this session was ever created from that profile, or that the
profile was ever used on that host. The profile chip the user sees on the session is itself the host's claim.

So a hostile host can label one of its sessions with any profile P, including one holding a key meant for another
machine. When the user clicks Replace on that session, the host receives P's raw command line.

SPEC.md says agents may discover profile names and ids, but this "does not extend to raw command lines or embedded
credentials". It also says a remote host must not gain access to secrets on the helm's machine. An open question for the
maintainer is whether profiles are treated as fleet-wide, and whether a user clicking Replace counts as consent.

Suggested fix: treat the reported `source_profile` as display metadata only. Relaunch from the session's own reported
`invocation`, unless the helm itself recorded creating that session id from that profile on that host. Or require
Replace to name the profile explicitly.

User-visible consequence: a compromised remote machine can obtain any saved profile's full command line, including
embedded keys, the first time you press Replace on one of its sessions.

Restater note: this finding adds no exposure beyond a path SPEC.md already accepts. The agent relay's `create` verb
accepts `--profile-id` / `--profile` and resolves it against the same helm-wide catalog (`agent_requests.rs:1032-1050`).
The target host may be the asking session's own host. The resolved bundle, including the invocation, is sent to that
target. So an agent or hostile supervisor on the remote host can already obtain any profile's command line by asking the
helm to create a session on its own host from that profile, with no user click. SPEC.md "Local authority and trust
between hosts" accepts agent-requested creation as a temporary execution exception. If disclosing profile invocations to
the create target is a defect, it sits in that accepted agent-create path (outside this audit area's scope), and fixing
Replace alone would not close it. The coordinator should weigh narrowing this finding to "Replace is an additional,
user-triggered channel for an exposure already reachable via agent create", or rejecting it on that basis.
