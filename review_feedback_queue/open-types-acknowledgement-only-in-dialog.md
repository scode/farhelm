# The helm accepts added open-with-click file types without the warning's acknowledgement

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

SPEC asks for an explicit, acknowledged save before the list of file types that open with a modifier-click changes, with
a warning for each risky type. Only the Settings dialog asks for that acknowledgement; the helm's preferences write
takes any parseable list, so a client other than that dialog could add a runnable type silently.

## Details

A lead raised by the independent drop check of the GUI migration's final review (Haiku pass, 2026-11-17), while it
checked the drop of H-P1-1 (opening downloaded files with the default app). Not investigated beyond the location below.
Suggested queue bucket **other**: no route to it outside the app's own dialog was established.

`crates/farhelm-helm/src/preferences.rs:120-132` (`put_preferences`) parses and normalizes `open_extensions` and stores
it; nothing there requires or records an acknowledgement. SPEC.md:2401-2408 says the field "requires an explicit,
acknowledged save" and that the dialog warns for each type outside the safe set. Agents have no route to this preference
(the agent request handlers in `crates/farhelm-helm/src/agent_requests.rs` and `approvals.rs` do not touch it), and the
helm's credential is handed only to the desktop app's own window (SPEC.md, "Security"), so today the only writer is the
Settings dialog, which does ask.

What would move it up: any shipped client of the helm's preferences route other than that dialog (a future remote helm's
second UI, a command-line verb) that can write `open_extensions`. A fix either enforces the acknowledgement at the helm
(for example by refusing a widened list without an acknowledgement flag) or records in SPEC that the acknowledgement is
the dialog's alone.
