# IME confirmation can save a template prematurely

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An IME confirmation could save a template before editing is finished.

## Details

F226 — **possible** — `crates/farhelm-ui/src/list/save_template.rs:208`;
`crates/farhelm-ui/src/list/save_template.rs:208–209` — IME confirmation can save a template prematurely

The template-name field handles every Enter by saving and stopping propagation, without checking whether composition is
active. If a supported browser/IME sends candidate-confirmation Enter while the draft is valid, it can persist the
unfinished template and close the launcher. That event sequence was not reproduced. Reject composing Enter before Save;
repeat suppression is a companion guard, while ordinary Enter-to-save should remain.

## Evidence and triage context

- crates/farhelm-ui/src/list/save_template.rs:207-210 invokes Save for every Enter and stops propagation without
  checking composition or repetition.
- crates/farhelm-ui/src/list/create_form.rs:3918-3923 guards composing or repeating Enter only at the parent form.
- crates/farhelm-ui/src/list/save_template.rs:157-193 performs the save when the current template passes validation.
- crates/farhelm-ui/src/list/view.rs:3420-3427 closes the launcher on successful saving.
- crates/farhelm-ui/src/list/save_template.rs:204–209: oninput updates the name; onkeydown checks only Key::Enter,
  prevents default, stops propagation and invokes Save.
- crates/farhelm-ui/src/list/save_template.rs:157–186: Save snapshots the name and selected fields, validates them,
  fetches existing names, writes the template and invokes on_saved on success.
- crates/farhelm-ui/src/list/templates.rs:417–434 and 454–459: failed name discovery and existing names refuse saving;
  these guards do not distinguish composing Enter from deliberate submission.
- crates/farhelm-ui/src/api.rs:2976–2986 and crates/farhelm-helm/src/templates.rs:47–55,80–112: the callback sends a PUT
  that validates and persists the template.
- crates/farhelm-ui/src/list/create_form.rs:3918–3923: the parent checks composing and repeating Enter, but the child's
  stopped event cannot reach this handler; preventing native submission would not undo the child's explicit Save call.
- crates/farhelm-ui/src/list/create_form.rs:4684–4688 forwards on_saved to on_template_saved;
  crates/farhelm-ui/src/list/view.rs:3420–3427 closes the launcher and opens the saved template.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"basis": "SPEC.md:554–573; SPEC_impl.md:3888–3895", "comparison": "These specify deliberate Enter-to-save behavior
  and successful-save navigation. They do not explicitly accept interpreting an IME conversion confirmation as
  submission."}
- {"basis": "TRIAGE_OUTCOMES.md:6957–6971, template-clobber.md", "comparison": "This covers overwriting an existing
  template when name discovery is unavailable. Its trigger, consequence and validation mechanism differ from premature
  composition-triggered saving."}
- {"basis": "review_feedback_queue/template-catalog-error.md:14–20; review_feedback_queue/template-dot-name.md:14–21",
  "comparison": "These cover failed template discovery and URL normalization of dot-only names, respectively. Neither
  covers keyboard composition or unintended successful saving."}

Caveats:

- Needs a supported engine/IME that exposes candidate-confirmation Enter with key Enter.
- No IME reproduction was performed.
- The name must already satisfy validation for a premature save to succeed.
- No browser reproduction or runtime tests were performed. The triggering browser/IME must deliver Enter during
  composition while the name and selected fields pass validation.
- This path saves a template; it does not launch or stop a session.
- Repeat suppression is a reasonable companion guard, but a separate repeat-induced defect was not established.
- TODO.md:33–43 contains no matching Planned item; BUGS.md contains no matching accepted bug. Targeted queue and ledger
  searches found no composition-specific disposition.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_11_cor:p1:F5`, `ui_desktop_11_sec:p2:F1`.

- `ui_desktop_11_cor:p1:F5`: confidence as filed: possible; suggested bucket as filed: other.
- `ui_desktop_11_sec:p2:F1`: confidence as filed: possible; suggested bucket as filed: other.
