# Narrowing Replace with can delete the source when Enter was shown as applying a template

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

Resizing Replace with can make Enter delete the source session while the visible template choice still offers to fill
the form.

## Details

**highest.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view/search.rs:241-256,543-555;
view/search/start.rs:102-142; view.rs:1283-1302,1333-1342,2136-2140; launcher/launch.rs:412-418;
crates/gui/farhelm-gui/src/prompt.rs:1149-1157,1193-1215; crates/farhelm-helm/src/sessions.rs:4863-4902,4983-5015.
Survives at 1a53d9d35889d7dde96e31320cc1681dd013737d view/search.rs:245-260,557-569.

In wide Replace with, Enter on a highlighted template applies draft edits. The live width test bypasses that target
after narrowing, and the pre-key narrow draw also clears it. GPUI may dispatch the key after the dirty draw and before
the frame is presented (SPEC_impl.md:984-988,1033-1035 at 1a53d9d35889d7dde96e31320cc1681dd013737d). The source-consent
snapshot is reused across a width-only change because the guard and unit offer are equal. It does not describe what
Enter does. On the ordinary valid route, the resulting Replace request creates a successor and calls delete_session_with
for the original; there is no intervening ordinary Replace question. The last presented template highlight offered no
deletion, even though the window has generic Replace wording and the source guard correctly names the original. This
meets the highest bucket for a destructive operation outside what the shown key action authorized
(SPEC.md:1433-1436,5572-5581). Confirmed from source and documented event order; native timing and frequency were not
measured. Bind Enter to the presented operation and consume the press when a resize invalidates it. ; Possible coverage:
same route as LS-04.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-command/LC-03`, with
`launcher-search/LS-04`, `launcher-match-actions/resize-enter-falls-through`, `launcher-matches/LM-04`.
