# Git fixture commands can modify the caller’s repository

Reviewed commit: 3d0bd43a7f9cf2f87adcb0077f011e9d5ddcb05d

## TLDR

Running the new browser test with an inherited Git repository selector can change a developer’s repository instead of
its temporary fixture.

## Details

Suggested bucket: `highest`. Confidence: confirmed for the wrong-repository write by static tracing; potentially-highest
classification is conservative because it modifies user-owned repository configuration. No runtime reproduction or
destructive consequence demonstrated. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

e2e/tests/github-checkout-composer.spec.ts:131,152–155 uses execFile without a child-specific environment. An absolute
GIT_DIR pointing to an existing repository overrides the intended repository selected by -C: init reinitializes that
repository, and remote add writes the fixture origin there if origin is absent. The following config assertion reads the
same unintended repository and therefore does not establish fixture isolation. The recorder preserves Git variables
(scripts/record-test-run.py:1223–1230); start-stack.sh:230–234 scrubs them only in the supervisor’s login profile, not
the Playwright worker. Scrub repository-selecting and GIT_CONFIG_* variables on these Git children, without changing
process.env. Scope: developer tooling; no shipped launcher mutation or deletion demonstrated.

Possible cover: TRIAGE_OUTCOMES.md, git-fixture-setup-modify-delete-callers-repository.md (7244–7258), records the same
isolation problem in Rust repository-discovery fixtures, fixed by #1798. It does not cover these newly added
browser-test Git children.

Current-main status: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. The entire browser-test file is
unchanged between the reviewed commit and pinned main; no fixing commit found.

The full commit diff and relevant failure paths were read. No runtime reproduction was run.
