# Template summaries interpolate model text into approval wording without escaping — Templates-list summary rendering

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The templates list could display misleading permission wording in summaries.

## Details

F49 — **possible** — `crates/farhelm-ui/src/list/templates.rs:1134` — Template summaries interpolate model text into
approval wording without escaping — Templates-list summary rendering

The templates-list summary renders model text together with the application's approval and trust wording, without
escaping or isolating that value. Directional model text can affect the apparent surrounding wording, though no visual
deception or permission change has been demonstrated. Selecting a template still opens the launcher and its checks.
Render an escaped, independently direction-isolated model value here, using the shared summary correction rather than
relying on the earlier safe save preview.

## Evidence and triage context

- crates/farhelm-ui/src/list/templates.rs:477–492: template_summary inserts model.clone() and subsequently appends
  effort, approvals and trust wording.
- crates/farhelm-ui/src/list/templates.rs:1134: the Templates list renders the whole summary without display_peer or
  per-value isolation.
- crates/farhelm-ui/src/list/quick_switcher.rs:323–325: the quick switcher escapes the template name but places the raw
  summary inside one outer bdi.
- crates/farhelm-ui/src/list/templates.rs:228–262 and crates/farhelm-proto/src/launcher.rs:320–342: model text is not
  rejected for presentation-unsafe characters.
- crates/farhelm-ui/src/list/create_form.rs:2616–2626,295–300,4662–4668 and
  crates/farhelm-ui/src/list/save_template.rs:48–50: a cloned session's model can pass through the raw launcher snapshot
  into a saved template.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:3909 allows unrestricted model text, which establishes accepted input rather than permission to render it
  ambiguously.
- SPEC.md:2206–2209 excludes hypothetical browser hardening; this finding identifies an actual raw interpolation site,
  though its practical exploitability remains unverified.

Caveats:

- No permission value changes merely from rendering the summary.
- Template selection still opens the launcher, and launch validation and applicable confirmations remain.
- No visual exploit reproduction was performed.
- A safe preview during template saving reduces exposure but does not escape subsequent summaries.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_11_sec:p1:F3`.

- `ui_desktop_11_sec:p1:F3`: confidence as filed: possible; suggested bucket as filed: highest.
