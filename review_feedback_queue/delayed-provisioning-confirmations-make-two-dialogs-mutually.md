# Delayed provisioning confirmations can make two dialogs mutually inert

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A delayed uninstall confirmation can disable both open dialogs.

## Details

F106 — **definite** — `crates/farhelm-ui/src/modal_isolation.rs:121` — Delayed provisioning confirmations can make two
dialogs mutually inert

Host settings can open while uninstall planning is pending. When the plan returns, its confirmation mounts without
closing settings, and each dialog's independent isolation logic makes the other's subtree inert. Normal pointer and
focus interaction can then fail for both dialogs, even without a rapid user race. Coordinate modal ownership and defer
the arriving confirmation, or use shared stack-aware isolation that keeps the active dialog usable.

## Evidence and triage context

- crates/farhelm-ui/src/provisioning.rs:1948–1964 sets planning and awaits preparation without claiming the page
  operation token. Lines 1970–1976 install the returned pending plan and reveal details without coordinating settings
  ownership.
- crates/farhelm-ui/src/hosts.rs:3317–3338 permits Settings when row busy is false. Lines 1707–1723 check the operation
  token and provisioning-busy set, not pending planning.
- crates/farhelm-ui/src/hosts.rs:1633–1703 places the provisioning panel inside host-list. Lines 1788–1812 render
  HostSettingsDialog as a sibling of that list. The reveal callback at lines 1686–1689 closes menus, not settings.
- crates/farhelm-ui/src/hosts/settings_dialog.rs:422–432 installs settings isolation. Lines 157–162 and 193–215 install
  the separate uninstall dialog's isolation.
- crates/farhelm-ui/src/provisioning.rs:2789–2796 mounts the uninstall confirmation inside the provisioning panel.
- crates/farhelm-ui/src/modal_isolation.rs:115–129 marks off-path siblings inert without clearing inert attributes on
  the new dialog's ancestors. Settings therefore leaves host-list inert, while the arriving uninstall dialog marks the
  settings backdrop inert. Lines 132–144 install independent competing keyboard handlers.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- {"basis": "TODO.md:516–529, Maybe later, Native <dialog> for the app's modal dialogs", "comparison": "This is not a
  Planned item. It discusses converting other dialog surfaces and existing focus containment, not the delayed host
  confirmation overlap and mutual inertness."}
- {"basis": "SPEC_impl.md:428–438", "comparison": "The specification describes isolation that leaves the active dialog
  usable; it does not accept simultaneous dialogs making one another inert."}
- {"basis": "review_feedback_queue/FILTER.md:116–135", "comparison": "The interaction can occur throughout a slow SSH
  planning request, rather than requiring a person to win a self-closing sub-second window."}

Caveats:

- No browser reproduction was performed.
- The supported claim is mutual inertness and lost ordinary interaction, not that both dialog contents necessarily
  remain visually unobscured.
- Escape handlers may recover by cancelling dialogs. Permanent lockout or unauthorized uninstall is not established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_13_cor:p1:F2`.

- `ui_desktop_13_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: high.
