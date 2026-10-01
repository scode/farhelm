# A dropped create skips launch history and the remembered profile

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the page is reloaded or closed while a session is being created, the session still starts but never appears in the
create dialog's recent launches, and the dialog does not remember the profile that was just used.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F21 / COR-CREATE-BOOKKEEPING`, tagged **definite**. Anchor and title:
`crates/farhelm-helm/src/sessions.rs:1531` — a dropped create starts the agent but skips the helm's launch history and
remembered-profile writes.

Two reviewers found this independently. Creating a session from the GUI is one request to the supervisor followed by
three writes in the helm's own database (`accept_created_session`, `crates/farhelm-helm/src/sessions.rs`, around lines
1874-1929):

- the session is added to the helm's cached session list;
- an entry is added to launch history, which feeds the create dialog's recent folders and suggestions, and a successful
  write sends a notice to every open client;
- for a create made from a profile, the helm remembers that profile as the host's default for next time.

The create handler (`create_session`, line 1531) runs all of this on the HTTP request's own task, not through the
helm-owned `run_owned` helper described in F20. If the client disconnects mid-create (a page reload or close, or the
desktop app remounting its page after re-authentication), the web framework drops the task. The supervisor still
finishes creating the session and the agent runs, but the launch-history entry, its notice to clients, and the
remembered default profile are never written. The UI blocks navigation while a create is in flight, so a reload or close
is the realistic trigger. A fresh GitHub checkout widens the window, because the clone runs while the request is open.
The next periodic refresh repairs the session cache, but nothing repairs the other two.

The impact is low but permanent and silent: a launch missing from history, and a create-dialog default that does not
follow the profile the user just used. This breaks the rule in SPEC_impl.md's "Who owns an accepted action" section that
once the helm accepts a request, the client can lose only the reply, never part of the work. The suggested fix is to run
the create body, after request parsing, through `run_owned`. Ideally do it together with F3, since Replace reuses this
same create path and has the same defect.
