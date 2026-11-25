# SPEC_impl.md says custom agent IDs start with a letter; they may start with a digit

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The command-line reference in the implementation spec says registered custom agent IDs always start with a letter.
Farhelm actually accepts IDs that start with a digit (even an ID that is just a number), so the reference misdescribes
what `--agent` accepts. Nothing breaks for a user, but a maintainer relying on the sentence would be misled.

## Details

Concerns f6e06c88c2. SPEC_impl.md:7549-7550: "`--agent` accepts built-in names and registered IDs, which start with a
letter."

The ID rule is `farhelm_proto::custom_agents::validate_id` (`crates/farhelm-proto/src/custom_agents.rs:37-54`): the
first byte must satisfy `alnum`, defined as `is_ascii_lowercase() || is_ascii_digit()` (line 38). The unit test
`custom_ids_exclude_existing_cli_choices` (`custom_agents.rs:94-97`) asserts that "1" is a valid ID, tying the rule to
the published custom-agent contract. SPEC.md:489 says "lowercase letters or digits with internal hyphens". The CLI's
`parse_agent_type` (`crates/farhelm/src/main.rs:910-935`) accepts any token `validate_id` accepts. The sentence's
purpose (explaining why `--agent` needs no `allow_hyphen_values`) still holds, because IDs cannot start with `-`.

Fix: "which never start with a hyphen", or "which start with a lowercase letter or digit".

Bucket: other (spec accuracy only; no user-visible behavior is wrong). Not highest: no security or data consequence.

Confidence: confirmed from source.

Possible cover: SPEC.md:489 states the correct rule, so a careful reader can reconcile it.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g25 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
