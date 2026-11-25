# Recipe launches into the new home folder skip the early title check

Reviewed commit: 1eb6ae765a47327801e50393b737050ccb16ce9b

## TLDR

Farhelm now refuses a bad session name before it starts a new sandbox or container, but only when the launch also clones
a repository. A recipe launch into the new environment's home folder with a name holding a control character (for
example a tab in an agent's requested session name) still creates the sandbox or container first. The session create
inside it is then refused, and the launch fails after the user approved and waited for, and for Tensorlake paid for, an
environment that is then thrown away.

## Details

At 1eb6ae765a, `CreateReq::prepare_sandbox_launch` (`crates/farhelm-helm/src/sessions.rs:1384-1435`; 1196-1245 at the
tip) runs `check_checkout_preview_title` and `checkout_basename` only inside
`if let Some(checkout) = &self.github_checkout`. The else branch checks only cwd. Both callers then provision:
`provisioning/tensorlake_add.rs:349` (`start_session_environment`) and `agent_requests.rs:4639`
(`launch_environment_for_agent`). The title is first judged by the supervisor's `ensure_title_printable`
(`farhelm-supervisor/src/service/core.rs:4035`) inside the new environment.

Input routes the reviewer checked: saved templates are closed, because `farhelm_proto::launcher` refuses control and
presentation-unsafe characters in a template's session name at save (`launcher.rs:623-641`). The agent route looks open:
`ParsedAgentVerb` parsing of `CreateEnvironment` (`agent_requests.rs:1388-1400`) validates only recipe and container
host, `plan_environment_create` (`agent_requests.rs:4354-4440`) passes the title through, and `agent_launch.rs`
validates only the initial prompt. The refusal is environment-independent
(`farhelm_proto::text::check_session_title_printable` plus the 64 KiB bound), so it can run before provisioning as the
commit does for checkouts.

Fix: in `prepare_sandbox_launch`, check `self.title` whenever it is set, not only under `github_checkout`; add a test
beside `provisioning/tensorlake_add_tests.rs:729-759` without a checkout and with a control-character title, asserting
400 and no backend events, plus an agent-request variant.

Suggested bucket: `other` (a wasted, briefly billed environment and a failed launch with a clear reason). It would move
to `high` if a common GUI input produced such titles. Confidence: likely. The missing helm-side check is confirmed in
source; unverified after investigation that the agent CLI delivers a control-character name unchanged to this path (the
CLI side and template edit handling were not read in full, nor the GUI name field's paste filtering). Possible cover:
SPEC.md's new managed-checkout sentence promises the early check only for "the display title and first checkout
directory name", which triage may read as scoping the guarantee to checkout launches.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g06-podman-cleanup-checkout-names` record `f2`.
