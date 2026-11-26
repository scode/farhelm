# Claude trust added in a fresh checkout on an older supervisor survives archive

Reviewed commit: 72ab31157fcec9c5c7145059c28011426a1274e0

## TLDR

With the Claude trust switch on, a Claude Code session started in a new managed checkout on a host whose supervisor has
not yet been updated gets the folder marked trusted in Claude's settings, but Farhelm keeps no note to undo it. Deleting
the session archives the checkout and leaves the trust entry. The next unnamed checkout of a repository with the same
name reuses the lowest free `repo-N` path, so it starts out trusted: Claude runs that repository's hooks and commands
without asking, even if the user has since turned the switch off, launched Claude through a Command template, or the new
checkout is a different owner's repository of the same name (a fork vs. upstream). Before this commit the same
helm/supervisor pair recorded the trust for removal.

## Details

Priority bucket: `highest`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t05a-trust-hooks-backend` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Bucket raised from the reviewer's suggested `high` to `highest`: the consequence is a trust-boundary one (Claude runs a
repository's hooks and commands without its trust question), and the route is established in shipped code (mixed
helm/supervisor versions are a normal steady state per SPEC.md "Errors and diagnostics"; unnamed checkouts reuse the
lowest free `repo-N` from the repository name alone, `crates/farhelm-proto/src/github_checkout.rs:333-372`). The
reviewer's argument for `high` rests on SPEC.md "Managed checkouts" accepting trust from a later launch; the independent
drop check found that sentence does not cover the initial launch of a new managed checkout, for which the same paragraph
promises a best-effort removal. It would move to `high` if triage judges the archive-time removal a convenience rather
than a security guarantee. Coverage: the reviewer proposed a drop citing the SPEC_impl.md helm schema 53 / protocol 51
paragraph ("A supervisor older than that reads the missing field as no post-clone trust ..."); the independent drop
check DISAGREED (it documents the gap, and the 0529bd302a commit message leaves closing it for its own change), so it is
recorded. Not covered by TODO.md's Planned bucket, an unlanded plan, BUGS.md, the queue or TRIAGE_OUTCOMES.md (all empty
or unrelated at the tip). One premise the drop check did not check: that the checkout root has no per-owner
subdirectory.

### Reviewer record

Location: `crates/farhelm-helm/src/sessions.rs:1407`

At 72ab31157f the helm always sends `claude_trust: false` in the resolved checkout
(crates/farhelm-helm/src/sessions.rs:1407, `claude_trust: false`), and the field is then omitted on the wire
(crates/farhelm-proto/src/github_checkout.rs:515, `skip_serializing_if = Not::not`). No protocol bump accompanies this,
and SPEC.md "Errors and diagnostics" (SPEC.md:5135-5137 at the tip) says "Mixed versions across helm and supervisors are
a normal steady state, since each host is updated on its own schedule ... Incompatible versions refuse to connect with a
clear, actionable error; there is no silent degradation." Builds sharing a protocol version interoperate, so this
pairing connects and silently loses the archive removal, which that sentence rules out. An older supervisor (protocol 51
up to before #4992) builds `CheckoutPreparation.claude_trust = resolved.claude_trust` (false) and its shim runs
`if spec.claude_workspace_trust && !prep.claude_trust { apply_claude_workspace_trust(".", |_| Ok(())) }` (the pre-change
code removed at crates/farhelm-supervisor/src/launch.rs:1218-1227 in the snapshot). That callback writes no removal
record, so `remove_preparation_trust_sync` (launch.rs:1659-1682 at 72ab31157f) finds `record.claude_trust == None` at
archive and returns without touching Claude's file. Before #4992 the helm sent `claude_trust: true` with the switch on
and the old shim did the post-clone edit with the record. Route: helm updated, a host's supervisor not; Claude switch
on; launch a structured Claude session into a new managed checkout on that host (New, Clone, Replace with or
agent-requested); delete it (archive to `.farhelm-trash`); later launch another unnamed checkout of a same-named repo
there; SPEC.md "Managed checkouts" says an unnamed checkout uses the lowest available `repo-N`, so the path is reused
and Claude's `projects[<path>].hasTrustDialogAccepted` is still true. Verify by reading the old shim at
72ab31157f^:crates/farhelm-supervisor/src/launch.rs around exec_launch_spec_with_seam and run_checkout_preparation. Fix
shape (the commit message of 0529bd302a names it): keep asking supervisors that predate the change for the post-clone
edit (send `claude_trust: true`) for exactly the structured Claude launches with trust on; the new shim already ignores
that flag. That changes what new creates' fingerprints contain, so it needs care with keyed retries; alternatively gate
fresh-checkout Claude trust on a supervisor capability/protocol check, or have new-helm archive remove the key using the
launch's resolved path when no record exists.

Suggested bucket: high. A stale trust grant lets a repository's hooks run without the trust question; it stays out of
highest because SPEC.md "Managed checkouts" already accepts trust entries that outlive archive in the same reusable
paths ("Trust added by a later launch in a checkout or by a launch in a directory the user chose stays"), so this widens
an accepted class rather than opening a new one, and the transitional gap is documented. It would move to highest if the
maintainer considers the archive-removal promise for initial launches a security guarantee (the different-owner
same-name repo case is a genuine untrusted-hooks route); it would move to other if the same-repo case is considered the
only realistic reuse.

Confidence: confirmed. Missing record and archive skip on older supervisors: source at 72ab31157f^ and 72ab31157f. Path
reuse: crates/farhelm-proto/src/github_checkout.rs:333-335 and 359-372 (tip) build an unnamed checkout basename from the
repository NAME component only (owner ignored) as the lowest unused `repo-N`, and the trash move frees the original
path, so `me/project` and `org/project` both land on `project-1` once the first is archived.

Also user-facing: the Settings caution for Claude (crates/gui/farhelm-gui/src/settings/agent_trust.rs:55-57, tip,
introduced by this commit) says "When Farhelm adds trust on the first launch in a new managed checkout, it tries to
remove the entry when it archives the checkout." On an older supervisor Farhelm does not try, so the caution the user
relies on when opting in overstates the cleanup; a fix should either close the gap or qualify that text.

proposed_drop: SPEC_impl.md (Helm schema 53 / Protocol 51 paragraph, added by 0529bd302a) states the behavior as known:
"A supervisor older than that reads the missing field as no post-clone trust and still applies a structured Claude
launch's trust, but without recording it for removal, so until it updates, a Claude launch into a fresh checkout on it
leaves a trust entry that archiving the checkout does not remove." The commit message of 0529bd302a says closing it is
"left for its own change". It documents the gap rather than deciding it is acceptable, so I do not think this alone
justifies a drop; the commissioner decides.

possible_cover: SPEC_impl.md paragraph quoted above; the changelog draft releasing/changelog.d/claude-trust-at-launch.md
(edited by 0529bd302a); possibly a TODO entry for the follow-up the commit message mentions (not searched, per brief).

Revision: corrected the SPEC.md section citation (Errors and diagnostics, not Agents and environments) and added its
no-silent-degradation sentence.

Revision 2 (synthesis): path-reuse premise verified in the allocator source, confidence raised to confirmed; added the
Settings caution wording as part of the user-visible impact. Bucket unchanged (high).
