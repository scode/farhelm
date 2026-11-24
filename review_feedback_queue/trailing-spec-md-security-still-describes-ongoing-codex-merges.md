# SPEC.md Security section still describes ongoing Codex settings merges after #4679/#4680 replaced them with one-time cleanup

Reviewed commit: 360e35c6e088205d01ca8e83382ac5e102d5ab22

## TLDR

The Security section of SPEC.md contradicts the Topology section by still stating that Farhelm performs ongoing Codex
settings merges that can grow ~/.codex/config.toml past 1 MiB and cause later merges to refuse the file.

## Details

**other.** Source inspection. Main reference `SPEC.md:4978-4980`.

Commit 360e35c6e088205d01ca8e83382ac5e102d5ab22 updated SPEC.md (lines 434-435 and 537-544 at 360e35c6; lines 441-442
and 561-568 at review tip 1b3fe49af86979aebc2719965682a17f8c6366ac) and SPEC_impl.md (lines 6028-6050 and 6585-6599 at
360e35c6; lines 6093-6115 and 6650-6664 at review tip) to state that Codex recipe endpoints are supplied at launch and
that delivery performs only a one-time legacy cleanup of ~/.codex/config.toml (removing only model_provider="farhelm"
and experimental_bearer_token) before leaving ~/.codex/config.toml alone. However, SPEC.md lines 4978-4980 at 360e35c6
(lines 5034-5036 at review tip, under Security) were not updated and still read: "The Codex settings merge and one-time
Claude settings cleanup read files of at most 1 MiB. A Codex merge can grow a file near that size past it. Later merges
then refuse the file until the user reduces its size." Verify by inspecting SPEC.md:4978-4980 (at 360e35c6) /
SPEC.md:5034-5036 (at 1b3fe49af86979aebc2719965682a17f8c6366ac) and
crates/farhelm-helm/src/provisioning/agent_endpoints.rs:96-124 (clear_codex only removes keys; there is no ongoing Codex
settings merge). Fix by updating that paragraph in SPEC.md to describe the one-time legacy Claude and Codex settings
cleanups reading files up to 1 MiB and removing the obsolete "A Codex merge can grow a file near that size past it.
Later merges then refuse..." wording.

Reviewer metadata (g23_codex_recipe_creds_sup_spec/f1): suggested bucket: other (internal specification drift; does not
lead a reader into a security or data-loss mistake).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g23_codex_recipe_creds_sup_spec/f1`.
