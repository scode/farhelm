# Sidebar directories can visually disguise the session’s actual folder

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Invisible characters can disguise a session's folder in the sidebar.

## Details

F45 — **definite** — `crates/farhelm-ui/src/list/row.rs:1672` — Sidebar directories can visually disguise the session’s
actual folder

The visible abbreviated folder is rendered directly from supervisor-supplied text. Invisible characters can make it
resemble another folder, affecting the information users rely on when selecting sessions for lifecycle actions.
Direction isolation and an escaped tooltip do not expose those characters in the ordinary row. This is visual spoofing,
not executable markup. Apply the existing peer-text display escaping to the visible folder, retaining direction
isolation and the full escaped tooltip.

## Evidence and triage context

- crates/farhelm-helm/src/manager.rs:730–771: listing ingestion checks count, session identifiers and duplicate
  identifiers, then returns the listing without sanitizing cwd.
- crates/farhelm-ui/src/list/row.rs:365–392: abbreviate_home shortens selected home prefixes but otherwise preserves
  characters.
- crates/farhelm-ui/src/list/row.rs:892: cwd_shown is computed directly from session.cwd.
- crates/farhelm-ui/src/list/row.rs:1671–1672: the tooltip uses display_peer, but the visible child renders cwd_shown
  directly with dir=ltr.
- crates/farhelm-ui/src/list/row.rs:1903–1935: Delete confirms the title and submits the selected session's guard; it
  does not independently disambiguate a misleading folder.
- crates/farhelm-helm/src/manager.rs:730-771 validates listing size and session identifiers but passes cwd through
  unchanged.
- crates/farhelm-ui/src/list/row.rs:365-392 abbreviates home prefixes without escaping the remainder; line 892 assigns
  that result to cwd_shown.
- crates/farhelm-ui/src/list/row.rs:1671-1672 escapes the tooltip but directly renders cwd_shown.
- crates/farhelm-ui/assets/app.css:3434-3443 controls clipping direction; it does not escape characters.
- crates/farhelm-ui/src/peer.rs:63-84 supplies the escaping this visible text bypasses.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:2290–2311, session-header-raw-peer-text.md: matching spoofing mechanism, but explicitly concerns
  the session header.
- TRIAGE_OUTCOMES.md:2314–2330, titles-raw-in-confirm-prompts.md: concerns sidebar titles and confirmation quotes, not
  sidebar directories.
- TRIAGE_OUTCOMES.md:2290-2312 covers the session header, including its directory and copy controls.
- TRIAGE_OUTCOMES.md:2314-2331 covers sidebar titles and confirmation quotes, not sidebar directory text.

Caveats:

- The rendering defect is definite; resulting mistaken selection depends on user reliance on the folder.
- This is visual spoofing, not executable markup injection.
- The tooltip and compact-mode accessible folder are escaped.
- No browser reproduction was performed.
- Visual spoofing, not HTML injection.
- The tooltip is escaped and compact mode's hidden directory is escaped.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_11_sec:p1:F1`, `ui_desktop_11_cor:p1:F1`.

- `ui_desktop_11_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `ui_desktop_11_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
