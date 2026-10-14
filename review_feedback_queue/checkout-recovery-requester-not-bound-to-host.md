# Checkout recovery trusts a remote-supplied requester identity

Reviewed commit: c8de519d6351af429a9c2ff519bc54774a97b8ed

## TLDR

A hostile attached host can impersonate another host’s session and recover its approved checkout attempt without an
approval for the hostile host.

## Details

Suggested bucket: `highest`. Confidence: likely: the missing origin binding is confirmed by inspection; exploitation
requires knowing or guessing the original key/request and an accepted attempt eligible for launch recovery. No runtime
reproduction.. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

Introduced at crates/farhelm-helm/src/agent_requests.rs:1357–1369: reconciliation runs before approve_launch. The
reservation key uses only the supplied asking_session and key hash (1150–1157), while the request digest contains no
authenticated origin host (858–932). The helm accepts the supervisor-supplied session_id without ownership verification
(crates/farhelm-helm/src/client.rs:1958–1976); handle checks only that the sending connection is live
(agent_requests.rs:294–309). Therefore host B can send host A’s requester ID and repeat its target, key and request. The
target compares the key and request identity, then can finish an interrupted launch from its stored snapshot
(crates/farhelm-supervisor/src/service/core.rs:7054–7079). B’s commands-without-asking setting is never consulted on
this branch. Scope: previously approved managed creates; this does not permit arbitrary new launch contents. Bind
recovery authority to the helm-known requesting host/installation as well as the claimed session.

Possible cover: SPEC.md:2338–2340 permits recovery only for the session that asked; it does not permit another host to
impersonate that session. TRIAGE_OUTCOMES.md heading agent-create-replays-asker-as-child.md covers ordinary session key
scoping, not hostile supervisor impersonation across hosts.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: agent_requests.rs:1363–1375,
client.rs:1958–1976 and supervisor service/core.rs:7077–7102 retain the same mechanism. No fixing commit identified.

Review coverage: full commit diff and relevant code paths read. Runtime behavior and harmful reliance were not
reproduced; the note preserves the reviewer's stated unverified premises.
