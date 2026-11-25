# No helm test that sandboxes are never offered checkout-folder setup

Reviewed commit: b2f51bbeea61da91f272e27d061365c91e1f61fa

## TLDR

SPEC.md says a session's own sandbox or container uses its home as the checkout-folder fallback and needs no setup.
Before this change a helm test checked that the launcher was told "no setup needed" for such a host. This change moved
that answer to a different reply and deleted the check without adding one for the new reply, so a regression that makes
the launcher ask sandbox users to set up a checkout folder would pass the helm's tests.

## Details

At b2f51bbeea the `needs_checkout_root` flag moved from the removed `POST /api/github-repositories` reply to
`GET /api/launch-history` (`crates/farhelm-helm/src/sessions.rs` `launch_history`, ~558-596 at the tip, computing
`config.root.is_none()` from `checkout_config_for`, which applies the `~` fallback only for session-owned hosts,
sessions.rs:204-219). The commit deleted the assertion `discovery["needs_checkout_root"] == false` for the session-owned
host from `assert_new_environment_checkout` (sessions_tests.rs, old line ~677). Its replacement test,
`launch_history_reports_the_effective_checkout_folder_requirement` (sessions_tests.rs:12339), uses only the local
installed host. No other helm or e2e test reads `needs_checkout_root` from launch history for a session-owned host, and
the GUI has no independent guard (`launcher/view/checkout.rs:302-306` takes the flag as-is; GUI tests stub the reply).

Fix: in `assert_new_environment_checkout` or a sibling, GET `/api/launch-history?host={owned}` and assert
`needs_checkout_root == false` while the global root is unset.

Suggested bucket: `other` (coverage gap; the guarded behavior is a setup-prompt nuisance). Confidence: confirmed from
source, not executed.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g09-checkout-history-proto-spec-tests` record `f3`.
