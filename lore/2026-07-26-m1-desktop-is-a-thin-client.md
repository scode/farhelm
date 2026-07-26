# M1 desktop target is a thin client over FARHELM_URL, not an embedded helm

Decision (2026-07-26, during the M1 walking-skeleton build): the desktop
target of farhelm-ui is a wry window rendering the same components as the
web build, pointed at an already-running helm via the FARHELM_URL
environment variable (default http://127.0.0.1:7433). It does not embed
the helm or a supervisor in-process.

Why now, and why it could have gone the other way: SPEC.md's end state is
a native app that embeds helm + local supervisor, and PLAN_M1.md's
"desktop window" bullet could be read as requiring that embedding.
Embedding in M1 would mean running a tokio runtime alongside the wry
event loop, lifecycle-managing the supervisor from the GUI process, and
packaging decisions — none of which the walking skeleton needs to retire
its actual risks (Dioxus dual-target, xterm island, byte path). The thin
client validates the dual-target story at near-zero cost; PLAN_M1.md
explicitly blesses "dx serve-grade" desktop.

Revisit when: the Mac app packaging milestone (M7 in PLAN.md's ladder)
starts, or earlier if dogfooding on Linux desktop wants a one-command
launch. The embedding work is additive — the components and the API
client don't change — but the FARHELM_URL mechanism should be replaced,
not kept alongside, when the app owns its helm.
