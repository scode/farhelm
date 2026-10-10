# overlapping uninstall dialogs can confirm the wrong host

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Overlapping uninstall dialogs could route keyboard confirmation to a hidden host plan.

## Details

F43 — **possible** — `crates/farhelm-ui/src/provisioning.rs:2796` — overlapping uninstall dialogs can confirm the wrong
host

Independent host plans can mount separate confirmation dialogs, but each focus handler selects the first matching dialog
across the document. One visible plan can therefore cover another dialog that still receives keyboard input. Paint and
focus behavior for the full wrong-host sequence remain unverified; initial focus is Cancel, so initial Enter alone is
not the trigger. Serialize provisioning confirmations and bind focus and isolation to the actual mounted dialog
instance.

## Evidence and triage context

- crates/farhelm-ui/src/hosts.rs:1673–1703 mounts a separate provisioning panel per host. Lines 1744–1763 check the
  selected host and page operation token, without globally serializing pending plans.
- crates/farhelm-ui/src/provisioning.rs:1781–1783 stores pending/planning state per panel; 1948–1964 starts planning
  without acquiring the page token; 1970–1976 independently publishes each completed plan.
- crates/farhelm-ui/src/provisioning.rs:2783–2808 mounts each uninstall dialog and invokes the shared installer.
- crates/farhelm-ui/src/hosts/settings_dialog.rs:54,157–162,202–215 uses document.querySelector with the same uninstall
  selector for focus and isolation.
- crates/farhelm-ui/src/modal_isolation.rs:108–109 reuses the first matched dialog and skips installing isolation twice;
  121–130 makes other host subtrees inert without hiding them.
- crates/farhelm-ui/assets/app.css:2399–2407 gives each backdrop the same fixed positioning and z-index.
- crates/farhelm-ui/src/provisioning.rs:2523–2533 validates the focused panel's pending plan and claims the token, but
  does not establish that this is the visible dialog. Lines 1047–1053 submit that panel's host ID and plan token.
- SPEC.md:130–137 and SPEC_impl.md:3341–3342 require the user to see and confirm the removal plan.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:6855–6868 checks cancellation and consumption of each panel's own question. It does not address
  simultaneous dialogs, document-wide selection, or confirming a visually obscured host.

Caveats:

- The selector chooses the first dialog in document order, not necessarily the first response chronologically.
- The wrong-host keyboard-confirmation sequence requires browser verification of paint order and focus; none was run.
- Initial focus is Cancel, so a bare initial Enter is not itself the destructive trigger; subsequent keyboard navigation
  can reach Confirm.
- Backend plan and liveness checks still apply. Actual session termination or retained-data deletion was not
  established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_13_sec:p1:F1`.

- `ui_desktop_13_sec:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
