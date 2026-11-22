# A launcher edit can silently swallow a refused hook settings save

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Editing the launcher while a hook setting saves can hide a later refusal.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/view.rs:942-974,2220-2250;
launcher/view/hook_question.rs:94-104,107-128,229-255; prompt.rs:669-709; operations.rs:900-927; SPEC.md:5567-5570;
SPEC_impl.md:1039-1043

If the user edits a still-open launcher field while Set up and launch or Don't ask again is saving, the hook question
vanishes. If the helm refuses the write, the user sees no reason and the preference does not take effect. Name remains
editable during hook_write_in_flight and its edit calls changed. Retiring the launch continuation on edit is
appropriate, but retire_hook_answer also drops the inline presentation; the refusal is sent only to Attempt::failed for
that weak prompt. Operations retains the refused result under its generic preferences key, which the launcher never
draws. This route needs no launch, host or session change, and differs from the Settings-page close/disable refusal in
HS-8. Confirmed source path conditional on an in-flight reply being refused. Suggested other for silent failure to save
global setup or dismissal; no highest consequence established. Preserve an attributable error in the launcher even when
the original draft can no longer continue. Possible coverage: hook-settings HS-8 is a different caller.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-launcher/HL-13`.
