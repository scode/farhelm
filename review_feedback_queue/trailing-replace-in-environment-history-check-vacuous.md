# Replace-in-environment test no longer checks that the host's history is left alone

Reviewed commit: b2f51bbeea61da91f272e27d061365c91e1f61fa

## TLDR

SPEC_impl.md promises that a fresh checkout started inside an existing sandbox or container (for example by Replace
with) adds its repository to the suggestions without aging that environment's earlier launch setup. The only test that
drives this through the helm's real create route stopped checking it in this commit: it now checks a list this test's
launch can never write, so it passes even if the helm records the replacement as an ordinary launch.

## Details

In `fresh_replace_on_a_session_owned_host_uses_its_home_without_setup` (`crates/farhelm-helm/src/sessions_tests.rs`,
assertion at 847 at b2f51bbeea, 880 at the tip), the doc says "The new checkout teaches the global repository list
without changing host suggestions." Before this commit the closing assertion read the host's create history table, which
the ordinary (`HistoryScope::All`) path writes and the `RepositoryOnly` path skips. b2f51bbeea replaced it with
`h.store.launch_history(host, "fresh-recipe-install").is_empty()`, which reads only the structured `launch_history`
table; `record_launch_history` writes nothing without an explicit agent selection (`store.rs:215-217`), and the
replacement is a command launch, so the assertion holds under both scopes. Flipping the decision in
`sessions.rs:2202-2208` to always `All` would leave the test green. The store-level test
`replacement_checkout_history_has_its_own_admission_window` (`store.rs:10725`) calls the store with `RepositoryOnly`
directly, so it does not cover the route's choice. A later commit (dd48d43d0) changed this test but did not restore the
check.

If the regression happened, each fresh checkout in a long-lived environment would take a slot in its 100-entry create
window and could evict the environment's original recent setup, contrary to SPEC_impl.md "Schema 102" ("without aging
the host's earlier setup or changing remembered settings").

Fix: seed the host with one structured create before the replace, or make the replacement an agent launch, and assert
that the host's launch history and ordinary create rows are unchanged; or keep a store read of the host's create
history.

Suggested bucket: `other` (coverage gap over suggestion convenience data). Confidence: confirmed from source, not
executed.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g09-checkout-history-proto-spec-tests` record `f2`.
