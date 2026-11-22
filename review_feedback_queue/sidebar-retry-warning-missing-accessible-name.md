# The retry row and attention summary omit the warning that the rail can name

Reviewed commit: fe63efdef0691208e093bb665ffe064deae511dd

## TLDR

A retrying host's warning appears as an amber edge without an equivalent in the session row's accessible description.

## Details

**other.** Source inspection.

fe63efdef0691208e093bb665ffe064deae511dd:
crates/gui/farhelm-gui/src/session_list/attention.rs:223-255,271-285,607-675,749-770,821-858,930-966,971-1025,1033-1046;
crates/gui/farhelm-gui/src/components/list_row.rs:138-157,174;
crates/gui/farhelm-gui/src/hosts/progress/card.rs:162-168,268-310; 1a53d9d35889d7dde96e31320cc1681dd013737d:
crates/gui/farhelm-gui/src/session_list/host_rail/status.rs:97; SPEC.md:2889-2916

during retries, a screen reader can identify the host and current work but does not get the unresolved warning from the
attention area. The consolidated button name and help share one sentence with the host identity, work verb, optional
current step, and open-Hosts hint. ListRow puts that name in aria_label and attaches the help as a separate input. The
visible identity line and progress indicator contain only the current status and progress; the area name and count-only
heading do not independently name the warning. The hover card describes the current run and connection, without
consulting the old warning. The problem branch does say 'Setup or update needs attention'. During work, row.warning
supplies the amber edge and its cue anchor without feeding any of those attention texts. In contrast, the separate host
rail status text can append 'setup or update needs attention; open Hosts for details' from the warning. Scope is the
attention area, not the rail. The spec requires an amber edge and prioritizes progress wording without prescribing an
accessible warning phrase. The reviewer suggested other, confirmed. Include the unresolved warning in the consolidated
working-row name or description while preserving its progress wording.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`session-sidebar/sidebar-retry-warning-accessibility`.
