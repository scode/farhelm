# Confirmed Delete does not have a regression assertion for a capture that reappears

Reviewed commit: 625e6893de5490b0d8a4fae300c4bb1fe1ec202f

## TLDR

The regression assertions would miss a change that forgets a capture when the confirmed request finds its image.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman/failure_tests.rs:1282`.

failure_tests.rs:1282–1366 asserts delayed absence, a follow-up marker, retention on an unanswered confirmed lookup, and
success after absence. The neighboring ordinary-retirement test podman/tests.rs:511–552 has a disappearing then present
tag and canonical removal, and :443–483 covers usage/non-forcing for an already canonical image, but those invoke
retire_images with consent false (build.rs:687–691). They do not assert that a confirmed route detecting a present tag
still uses the inspection, label, usage and non-forcing removal branch through build.rs:703–768,809. SPEC_impl.md at
f2b3b5b9c4be17779dbdce517c1a7af04c9f4eaa:6195–6198 explicitly requires the present branch. Extend the existing
confirmed-route assertions with a present result and ownership/usage predicates. Bucket: other, regression oracle only.
Confidence: confirmed by source and existing assertions, no runtime observation. Possible coverage:
unresolved_commit_absence_retains_ownership_until_positive_identity and
retired_images_wait_for_users_and_never_force_user_storage exercise adjacent unconfirmed retirement behavior.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recipes/f6`.
