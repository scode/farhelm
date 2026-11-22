# Busy-claim retry test bypasses the fleet wake it depends on

Reviewed commit: e3dc96fc7b1597c703c9fc24bd91dc4dd781e9da

## TLDR

The new test proves the planner can try again when explicitly called, while regressions that leave a host idle without
waking automatic Update can pass this test.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning.rs:1709`.

provisioning.rs:1696–1752 stops the listener, inserts/removes the busy set entry directly, manually calls scan a second
time, and witnesses consumption of a retry probe. It proves reservation removal and same-incarnation eligibility. It
never goes through either changed pre-run ADD release (service.rs:1523–1540) or checks its fleet bump, which is the
listener's only wake source (automatic.rs:43–68). SPEC.md:183–185 requires reconsidering a host when its work finishes.
Add a listener-level assertion tied to a real release path and without an unrelated fleet revision. Bucket: other (test
oracle). Confidence: source-confirmed scope of this test. Check separate existing listener assertions for possible
overlap. Possible coverage: automatic-update-counts-busy-as-failure (commit subject).

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `provision/p6`.
