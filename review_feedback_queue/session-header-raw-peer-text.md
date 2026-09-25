# Session header shows and copies peer text raw

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A session's command or directory can look harmless in the header while the copy button puts a different command on the
clipboard that runs when pasted into a shell.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F27 / SEC-HEADER-RAW-PEER-TEXT`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/session_view.rs:1537`,
`crates/farhelm-ui/src/session_view.rs:1551`, `crates/farhelm-ui/src/session_view.rs:1558`,
`crates/farhelm-ui/src/session_view.rs:1501` — The session header shows title, directory and command (and tooltips) raw
and copies the raw value, so what is shown can differ from what is copied

Farhelm has a rule for displaying text it did not write, implemented in `peer.rs`. Such **peer text** must go through
`display_peer`, which turns bidirectional-override and invisible characters into visible `<U+XXXX>` escapes, and it is
rendered inside an element with direction isolation (the `.peer-value` class or `dir="ltr"`). Dioxus already stops such
text from becoming HTML markup, so this is not about script injection. The concern is visual: a right-to-left override
(U+202E) or an isolate character can make text display in a different order from its actual bytes, and zero-width
characters can hide content. `peer.rs`'s module docs say "Everything a user sees has been through `display_peer`."

The session header does not follow that rule. The title (line 1537), the working directory and the invocation (the
command line that launched the agent, lines 1551 and 1558) are rendered, along with their `title` tooltips, as plain
interpolated text: no `display_peer`, no isolation. The two copy buttons (`copy_value`, line 1501) put the raw cwd and
invocation on the clipboard. These values come from the session's supervisor, which SPEC treats as untrusted when
reached over `--ssh`, and from agents, which can rename sessions and create sessions. The supervisor rejects only
`char::is_control` characters in titles, and cwd and invocation get only a length cap at the agent-create doorway. So
bidi overrides and isolates (U+202A–202E, U+2066–2069), zero-width characters, and for cwd and invocation even newlines,
all get through.

The invocation button is only wide enough for about 18 characters, so its tooltip is the only way to read the full
command. An embedded RLO can make that tooltip read differently from the bytes the button copies, and a newline renders
as a space but survives on the clipboard. The sidebar already escapes the same title in its tooltip ("Native tooltips do
not inherit DOM direction isolation", `row.rs:1519`), and SPEC_impl's clone rule requires these very fields to be shown
escaped in the clone form, because "a directional override or an invisible character [could] make the field say
something different from the bytes a submit would send".

This matters because the header offers "copy command", which a user is likely to paste into a shell. SPEC's "Local
authority and trust between hosts" says a remote host must not gain execution on the helm's machine, and "Remote input…"
rules out unsafe rendering of remote input. Harm still requires the user to paste and run what they copied, which is why
this is tagged "possible". The suggested fix is to render the three values and their tooltips through `display_peer`
inside isolated `.peer-value` elements; to keep copying the raw bytes; and to warn, or refuse, when the value contains a
character `must_escape` flags.
