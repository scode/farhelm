# Templates dialog's name check uses a stale list and overwrites a template created meanwhile

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

While the Templates dialog is open, it judges whether a name is taken from the list it read when it opened (or after its
own last save or delete). If an agent's approved command, a host allowed to run farhelm commands without asking, or the
user's own command line creates a template named, say, `review` in the meantime, and the user then creates a template
called `review` in the dialog or renames another one to `review`, the save goes through and silently replaces the other
template's definition (its command, prompt, model and so on). A rename then also deletes the old name, so two templates
become one. Nothing warns the user.

## Details

Found by the 2026-10-22 prioritized persistent review, group `181-ui-templates-2of2` (finding f1).

Mechanism:

- `crates/farhelm-ui/src/list/templates.rs:1467-1470` reads the template list once through a `use_resource` with no
  reactive dependency. Only the dialog's own save, delete, undo and retry restart it (`templates.restart()` around 1651,
  1680, 1697, 1705, 1726, 1812, 1868, 2059). Nothing ties it to the helm's change hint, and the caller
  (`crates/farhelm-ui/src/list/view.rs:4102-4111`) does not remount it on revision bumps.
- The save handler checks a new or renamed name only against that retained list: `name_refusal(templates, ...)` at
  `templates.rs:1628`, which returns `ListedNames::Loaded(...)` from `templates.peek()` whenever the resource is not
  pending (`templates.rs:1423-1437`). `save_name_refusal` (`templates.rs:884-903`) then answers "free" for any name
  absent from the old list.
- The write is `crate::api::put_template` (`templates.rs:1637`), the helm's unconditional create-or-replace
  (`crates/farhelm-helm/src/templates.rs:47-66` with `Precondition::None`; `Store::put_launch_template`,
  `crates/farhelm-helm/src/store.rs:4492-4519`, `INSERT ... ON CONFLICT (name) DO UPDATE`). For a rename the handler
  then deletes the old name (`templates.rs:1701-1709`).
- By contrast, the launcher's save-as-template panel and Delete's Undo read the list fresh right before the write
  (`crates/farhelm-ui/src/list/save_template.rs:218-223`, `templates.rs:1840-1857`), leaving only a millisecond window.
  The dialog's own save leaves a window as long as the dialog has been open.

Trigger: open Templates, then a second writer creates template `X` (`farhelm agent template create X` approved on its
card, since cards stay usable behind modal dialogs; or auto-run under the host's "run farhelm commands from this host
without asking" setting; or the CLI). Then create a new template, or rename an existing one, to `X` in the dialog and
save.

Why it looks like a contract violation: SPEC_impl.md "Templates" (around 5378): "saving under a name another template
already has is refused in the panel rather than overwriting that template." SPEC.md "One GUI at a time" (around 3324):
"A GUI write must not undo a change an agent or the command line made while that view was open". The earlier fix
recorded under TRIAGE_OUTCOMES.md `template-clobber.md` (around 6944) covered only a pending or failed list read, not a
loaded but stale one.

How to verify: in a browser test, open Templates with the list loaded, create template `X` with `PUT /api/templates/X`
directly (standing in for an agent or CLI write), then in the dialog start a new template, name it `X`, add a field and
save. The save succeeds and `GET /api/templates` shows the dialog's fields under `X`.

Fix sketch: for a new name or a rename, do what the save-as-template panel and Undo already do: declare a fresh
`fetch_templates` before the `put_template` future and refuse when the name is now taken. That leaves only the accepted
no-atomic-reservation window. Alternatively refresh the list on the helm's change hint.

Suggested bucket: the reviewer said high, possibly highest, because it silently replaces a stored template definition
another writer authored, which may hold a nontrivial command line or prompt. Recorded under highest so triage settles
the bucket; `template-preservation-promise-omits-last-write-wins.md` sits in the same bucket for a similar overwrite.

Confidence: confirmed by code inspection; no runtime reproduction.

Possible cover, for triage to judge:

- TRIAGE_OUTCOMES.md `template-preservation-promise-omits-last-write-wins.md` (decision 2026-10-21, planned in
  `plans/queue/gui-saves-last-write-wins.md`): "GUI saves of templates and number-key bindings are last-write-wins
  against agent and command-line edits ... that overwriting is accepted." That decision was about the editor saving its
  earlier draft over an agent's edit of the same template. Whether it also accepts the panel overwriting a different,
  newly created template, despite SPEC_impl.md's explicit "refused in the panel rather than overwriting" promise, is
  unclear, so recording did not treat it as covering this.
- SPEC_impl.md "Templates" (around 5384-5385): "API last-write-wins semantics remain unchanged; name checks do not
  acquire an atomic cross-client reservation." The reviewer reads this as speaking to the absence of an atomic
  reservation, not to checking against a list that may be minutes old.
- SPEC.md "Launch template" concept (around 62): "Template edits are last-write-wins: two clients editing the same
  template at once is not a case Farhelm guards". Here the dialog creates a different template, not the same one.
- FILTER.md "Races a person would have to win inside a sub-second window" does not match: the window is the dialog's
  whole open lifetime and the consequence is a lost template.
