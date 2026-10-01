# YOLO guard ignores the resume command a session will relaunch with

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

A profile can have one command for starting the agent and a separate command for resuming it. The confirmation for
approval-skipping launches on sensitive hosts looks only at the start command.

If a profile's resume command carries a flag such as `--dangerously-skip-permissions` and its start command does not,
this happens:

- New sessions from that profile start on a sensitive host with no confirmation, which is correct because the first
  launch is plain.
- The first Resume or Restart then relaunches the agent with approvals off, again with no confirmation.
- The sidebar never shows the YOLO badge.

A realistic way to get there: a user wrote a YOLO profile with an explicit resume command, the way the profile editor's
docs suggest (copy the start command and add `--resume {conversation}`). Later they remove the flag from the start
command to make the profile safe, and forget the resume field.

The same happens with `farhelm spawn` using a catalog profile. For a profile with no conversation integration, the
resume command is what Restart runs outright: "restart runs its configured resume command".

## Details

Source: gap-filling review pass, 2026-09-30, slice helm-store2.

Reviewer's confidence: likely (the helm and supervisor code paths are traced end to end; one premise is unverified: I
did not run a sensitive-host Resume/Restart to watch the approval-skipping command start).

Reviewer's bucket suggestion: highest.

Possible cover for triage to check: `yolo-guard-misses-equivalent-spellings.md`, `yolo-guard-misses-env-prefix.md` and
`yolo-guard-misses-codex-option-form.md` (the classifier's spelling gaps; at the time of the review all three were in
one item). It covers approval-skipping spellings in the _launch command line_: Cursor `-f`, `env` wrappers, Codex
`-a never -s danger-full-access`, and it declares arbitrary wrappers out of reach. This finding has a different trigger:
the separately stored _resume command_ is never classified at all. Also SPEC.md's Session creation YOLO paragraph: "A
plain restart relaunches the session's own stored launch and is not asked again". That sentence assumes the stored
launch was classified when the session was created. For the resume command it never was.

- **Only the start command is classified.** `crates/farhelm-helm/src/yolo_guard.rs:43-58`: `create_is_yolo` looks only
  at `invocation` (Raw), `compiled.selection` (Structured) or `profile.invocation` (ResolvedProfile). `resume_template`
  is never examined.
  - The create request's `resume_template` (sessions.rs `do_create_session`, forwarded at ~1717-1734) is not classified.
  - The resolved profile's `resume_template` (~1760-1778) is not classified either.
- **`farhelm spawn` has the same gap.** `crates/farhelm-helm/src/agent_requests.rs:339-375` (`ResolveProfile`) checks
  `invocation_is_yolo(&profile.invocation)` only, then returns `resume_template: profile.resume_template` to the asking
  supervisor.
- **Plain restart is never checked.** `crates/farhelm-helm/src/sessions.rs:2699-2714` (`do_restart_session`) sets
  `is_yolo` from the `with` selection only.
- **Nothing validates the template's content.** `crates/farhelm-proto/src/lib.rs:1255-1320` (`validate_profile_fields`)
  only requires a `{conversation}` element for integrated kinds.
  `claude --dangerously-skip-permissions --resume {conversation}` is a valid Claude template. The profile editor accepts
  it through `parse_resume` (`crates/farhelm-ui/src/profiles.rs:614-641`).
- **The supervisor runs the stored template.**
  - `crates/farhelm-supervisor/src/agent_kind/mod.rs:2268-2285` (`plain_id_offer`) offers `Resume` when a template and a
    captured id exist.
  - For a placeholder-free template it offers `FallbackTemplate` (`service/terminals.rs:447`: "restart runs its
    configured resume command").
  - `filled_resume_argv` (~2344) builds the relaunch argv directly from `self.resume_template`.
- **Consequence:** the session's first launch is plain, so the guard passes it. Every later Resume or Restart runs the
  template's approval-skipping argv on a host the user never marked safe, with no refusal and no badge. The badge also
  reads only the invocation (`farhelm-proto/src/yolo.rs` `invocation_marker`).
- **How to verify:** store a Claude profile with invocation `claude` and resume
  `claude --dangerously-skip-permissions --resume {conversation}`. Create from it on a default (sensitive) host: it is
  admitted. Resume after a conversation is captured: it runs the flagged argv. Or add a unit test showing
  `create_is_yolo` returns false for a `ResolvedProfile` whose template is YOLO.
- **Fix shape:**
  - Treat a create, clone, replace or `ResolveProfile` as YOLO when either the invocation or the explicit resume
    template satisfies `argv_is_yolo`. Apply this in `create_is_yolo`, the `ResolveProfile` arm, and any clone/replace
    mode derived from a source.
  - Optionally badge sessions whose template is YOLO.
  - Record in SPEC.md's YOLO paragraph that the resume command counts as part of the launch.
