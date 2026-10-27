# Matching Restart template model changes historical permissions

Reviewed commit: 25d0fe9c892b5a20ee67359e822d00be1fc8b3f5

## TLDR

A template repeating the current agent and model can change a session's historical agent-owned permission setting to
Guarded without containing a permission edit, making an otherwise unchanged Restart draft count as edited.

## Details

Matching kind/agent assignments are correctly removed at `crates/farhelm-ui/src/list/create_form.rs:607–615`. The
surviving model replays through `create_form/draft.rs:2311–2323` into whole-harness reconciliation at `891–909`.
`crates/farhelm-proto/src/launcher.rs:165` calls `request_permission`, whose omitted-mode fallback at
`launch.rs:293–300` converts historical omission to Guarded. Source seeding intentionally restores that omission at
`draft.rs:2712–2719`; ordinary same-harness model control at `870–889` preserves permissions. Validation performs the
same normalization and therefore accepts this change. This differs from the source-refuted unchanged-agent replay
candidate.

Suggested bucket: `other`. Confidence: mechanism confirmed, likely contract violation; no security escalation or
user-work loss established. Actual former vendor permission behavior would be a missing source premise for stronger
consequences. Possible cover: trailing-tip `SPEC_impl.md:3542–3545,5700–5717` describes reconciliation but does not
explicitly settle changing an omitted field; `SPEC.md:725–739` says absent template fields retain values and match hand
edits, and `SPEC_impl.md:585–588` preserves historical omissions. A full older-upgrade-plus-rare-condition filter match
was not established. Provenance: `a010/d1`. No vendor, browser or runtime reproduction ran.
