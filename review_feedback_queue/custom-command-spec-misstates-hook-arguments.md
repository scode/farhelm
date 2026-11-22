# Older custom-command wording conflicts with the newly documented persistent hook contract

Reviewed commit: 4603ee87fbf5d66aee13aaa16ba0854012432e6d

## TLDR

The custom-command contract says the launch placeholder supplies reporters for agents whose persistent configuration
supplies them.

## Details

**other.** Source inspection.

470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e, SPEC.md:1195-1199 and
website/src/content/docs/docs/how-it-works/agent-hook-injection.md:114-125; 4603ee87fbf5d66aee13aaa16ba0854012432e6d,
SPEC.md:3086-3090, 3149-3153, 3979-3986 and SPEC_impl.md:4503-4516; 1a53d9d35889d7dde96e31320cc1681dd013737d,
SPEC.md:1192-1203; website/src/content/docs/docs/how-it-works/agent-hook-injection.md:111-126;
crates/farhelm-supervisor/src/agent_kind/mod.rs:489-492, 579-595, 976-1008

The custom-command sections can send a Grok or Antigravity user to the mandatory placeholder when the reporter actually
comes from installed configuration. Scope correction: 4603ee87fbf5d66aee13aaa16ba0854012432e6d did not add the blanket
claims that the command's arguments turn integration on or that a declared agent gets a reporter at {farhelm_args}. Both
are present at the exclusive base, when manual saved callbacks already had no injected arguments for these kinds. The
changed product specification describes opt-in installation or update of persistent files, explicitly names that
exception, and now limits its placeholder troubleshooting advice to hooks passed as launch arguments. The changed hook
guide replaces the prior manual instructions with the automatic setup bullet. Those additions are correct. The remaining
issue is stale adjacent wording in the docs update, rather than an incorrect new product claim. Keep the placeholder
requirement and narrow the older generic promise to integrations that actually pass reporter arguments. The reviewer
suggested other. Confidence: confirmed text lineage and behavior.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`hook-contract/hook-custom-command-doc-injection`.
