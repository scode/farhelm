# Profile edits accept unknown fields and can wipe the resume template

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

A client that sends a profile update with a misspelled field name gets a success reply while the profile's saved resume
template is silently erased. Today's UI sends the right fields, so this needs a third-party or future client.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F27 / COR-PROFILE-UNKNOWN-FIELDS`, tagged **definite**. Anchor and title:
`crates/farhelm-helm/src/profiles.rs:116` — profile create/update silently accepts unknown fields, so a misspelled field
wipes the stored resume template.

Profiles are created and edited through the helm's REST API (`POST /api/profiles` and `POST /api/profiles/{id}`). Both
parse the body into `ProfileSpec` (`profiles.rs:116-129`), which has name, invocation, agent kind, and an optional
`resume_template`. The struct does not refuse unknown fields. Because `resume_template` is optional, a missing key means
"none", and an update replaces the whole stored profile. So a body that misspells the key (say `resumeTemplate`) or
carries any stray field gets a 200, and the profile's stored resume template is silently cleared. For an integrated
agent kind the supervisor then derives a default template from the invocation. For a generic profile, restart can only
start fresh.

Most neighbouring request bodies on the helm refuse unknown fields: host add, alias, YOLO-safe and adopt, the
preferences patch, clipboard, and client-log. The maintainer already chose to fix the same gap for host adoption
(`adopt-request-silently-ignores-unknown-fields.md`, outcome "fix code" in `TRIAGE_OUTCOMES.md`). Here a typo destroys
stored data rather than just being ignored. Severity is low: today's UI sends the correct field names, so only a
third-party or future client can trigger it. Suggested change: add `#[serde(deny_unknown_fields)]` to `ProfileSpec`.
Optionally, make `resume_template` a required key (with an explicit null for "none"), as the alias body does. Add a REST
test showing a misspelled key is refused and the stored profile is left unchanged.
