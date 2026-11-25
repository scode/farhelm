# Sandbox agents can plant repositories in the helm-wide checkout suggestions

Reviewed commit: 5795f9f035aa677f576a717baba1f37855483f3a

## TLDR

An agent running inside a sandbox or container session is meant to act only within its own environment. Since this
change, when such an agent starts a fresh GitHub checkout in its environment (`farhelm agent create --gh owner/repo`),
that repository goes to the top of the launcher's repository suggestions for every host, including the user's own
machine, and stays there after the sandbox is deleted. A compromised or prompt-injected sandbox agent can therefore put
a repository it chose, such as an ASCII lookalike of one the user works on, first in the Managed checkout list on hosts
it cannot otherwise reach. If the user takes that suggestion and launches, the attacker's repository is cloned onto the
user's host, the configured post-clone command runs in it, and so does trust after clone when the user has turned the
trust switches on.

## Details

b2f51bbeea61da91f272e27d061365c91e1f61fa (#4938) then made the list visible to every destination.

Recording path, confirmed from source at the tip: `record_created_history` (`crates/farhelm-helm/src/sessions.rs`, at
5795f9f035 ~2336-2392) has one origin gate, `if environment_target.is_some() && origin != CreateOrigin::User`, which
covers only a new environment's first launch. A create on an existing session-owned host has no `environment_target`, so
it passes for any origin, and the scope choice picks `HistoryScope::RepositoryOnly` for session-owned hosts with a
repository. `store.rs` then calls `repository_history::record_replacement`, which promotes the repository to the top of
the single `github_repository_history` table. The agent route is `agent_requests.rs::create_checkout_for_agent` →
`sessions::create_agent_checkout` (`environment_target: None`) → `create_fresh_session_resolved` →
`accept_created_session` → `record_created_history` with `CreateOrigin::Agent`. The agent picks the owner/name freely.

What a planted entry does: `GET /api/github-repositories` returns it to the GUI only (nothing leaks to agents). The
launcher loads the list for any destination and `gh:` search lists it in recency order. Taking a suggestion only fills
the destination; Launch still needs the checkout preview and an explicit press, and the row shows the real owner/name.
`parse_github_repo` limits names to ASCII, so Unicode homoglyphs are out but ASCII lookalikes are not. The entry never
reaches recent setups.

Spec: two reviewers reconciled this (one had first set it aside as spec-accepted and then withdrew that basis). For
acceptance, SPEC.md's managed checkout paragraph (~2138) says "Repository suggestions come from successful Managed
checkout launches across the helm", and SPEC_impl.md schema 102 says a checkout in a session-owned environment "can
contribute a repository"; neither names who started the launch. Against: SPEC.md "Agents and environments" (~4951), an
agent in a sandbox or container "can ask Farhelm to act, but only within its own environment"; SPEC_impl.md (~5391)
"only explicit GUI selections shape GUI defaults and suggestions" (attributed to SPEC.md, though no literal SPEC.md
sentence was found); SPEC_impl.md (~9552) agent checkout creates carry "no remembered GUI selection"; SPEC.md (~1203)
"Agent-origin launches and command launches do not change this memory". The code's own policy is inconsistent: an
agent-created new environment is skipped, while a confined agent's in-environment checkout is recorded. Before this
range, agent checkouts on ordinary hosts already fed that host's own recents; the new crossing is confined agents
reaching every host's launcher.

Fix: in `record_created_history`, skip the repository projection unless `origin == CreateOrigin::User` (at minimum for
session-owned origins), with a test that an agent-origin fresh checkout on a session-owned host leaves
`global_github_repository_history` unchanged. Or, if agent checkouts are meant to feed the list, say so in SPEC.md
beside "Agents and environments".

Suggested bucket: `high`. The established consequence is a sandbox-confined agent breaking the stated environment
boundary with persistent, top-ranked, attacker-chosen launcher entries on the user's other hosts. The code-execution
route needs the user to pick a planted lookalike and press Launch while the true name is shown, so per "Priority
buckets" (2026-10-22) it is bucketed by what is established. It would move to `highest` if a path takes a suggestion
without the user reading it, and to `other` if the maintainer rules agent-chosen repositories are intended suggestions.
Confidence: confirmed for recording and display from source; the harm route rests on a user choosing a planted entry.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g08-checkout-history-helm-store` record `f1` (duplicate after reconciliation: `g09-checkout-history-proto-spec-tests`
`c9`).
