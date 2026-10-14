# Setup still loses an environment-selected state directory

Reviewed commit: 07b7906519301d115802a8b388a43a3f37bb6663

## TLDR

Following setup’s sign-in advice can return an unusable token or create credentials for a different helm.

## Details

Suggested bucket: `high`. Confidence: confirmed by source; no runtime reproduction. Source: dedicated `gpt-6.1-sol` high
adversarial review. Proposed drop: none.

crates/farhelm/src/setup.rs:813–822 qualifies the command only when --state-dir was supplied. However, :636–639 also
selects a custom directory from XDG_STATE_HOME and pins it into both services. Trigger: run
`XDG_STATE_HOME=/srv/state farhelm helm setup`, then copy the printed command into the ordinary shell. The services use
/srv/state/farhelm, but token show resolves the ordinary shell’s default
(farhelm-supervisor/src/lib.rs:117–118,140–146). token_control.rs:211–218 can create a database and mint credentials
there. Print the resolved directory regardless of how it was selected. This is a remaining part of the pre-existing
advice bug, not a newly introduced regression; first-parent setup.rs:811–814 also omitted it.

Possible cover: TRIAGE_OUTCOMES.md heading setups-sign-in-command-ignores-selected-state-directory.md (:8813–8828)
explicitly describes setup --state-dir X, not the environment-selected trigger. SPEC_impl.md:3154–3158 explicitly
supports shell-only XDG_STATE_HOME. No matching current queue item or Planned item found.

Current-main status: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: setup.rs:636–639,813–822 retain
the same selection and advice condition. No fixing commit found.

The full commit diff and relevant failure paths were read. No runtime reproduction was run.

## Independent second-pass classification

A dedicated `gpt-6-astra` high review repeated this same environment-selected-directory/sign-in-advice finding and
suggested `other`; Sol suggested `high`. The source mismatch, pre-existing incomplete-fix scope and lack of runtime
reproduction agree. Both bucket suggestions are preserved for human triage. This adds no distinct finding or scope.
