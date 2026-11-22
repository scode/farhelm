# Each held-host template row clones hosts and rebuilds the full template snapshot

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

The wide launcher rebuilds the same draft data for every template row while it checks held fields.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view/search/start.rs:239–253;
crates/gui/farhelm-gui/src/templates/dispatch.rs:248–259; crates/gui/farhelm-gui/src/templates/apply.rs:108–162,373–431;
pinned 1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/view/search.rs:343–352,923;
crates/gui/farhelm-gui/src/launcher/view.rs:1143–1152

Redrawing the template choices in Replace with or Restart with does redundant work proportional to both the number of
rendered template rows and the current host list and form size. For each template row the held-host branch enumerates
and clones every ordered HostView into a Vec, looks up that template, scans for the selected host in the clone, and
rebuilds the full LauncherState. The snapshot copies the active prompt, model, command and opted-in resume text, parses
raw wrapper text when nonempty and custom permission flags when selected, and copies placement, destination and name.
held_refusal then clones that row's entire LaunchTemplate to strip equal held assignments; this execution does not fold
the template or validate its catalog choices. The shared literal conversation-selector check is not part of the row
snapshot; it runs on the disclosure and submit paths (pinned request.rs:530–560, more_options.rs:33–40). The wide empty
list walks every template (SPEC.md:1430–1439), and typed results that call row_disabled take the same held-host branch.
A once-per-draw held-value projection can share the host data and avoid rereading unneeded edit inputs without changing
application or the current pre-dispatch recheck. Repeated enumeration, allocation, copying and parsing are
source-confirmed. Noticeable latency, its threshold, and the frequency of large catalogs/forms remain unmeasured. The
reviewer suggested other.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-options/LO-08`.
