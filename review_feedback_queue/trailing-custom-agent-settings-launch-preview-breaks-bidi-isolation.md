# Custom-agent launch preview concatenates user command with flags inside one HostText, breaking bidi isolation

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

If a custom agent's command contains right-to-left characters, the Start and Resume command previews in Settings can
visually reorder Farhelm's --model, --effort, and --resume flags with the command text instead of keeping them isolated.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:634`.

At crates/gui/farhelm-gui/src/settings/custom_agents.rs:633-643, Editor::render formats the user/host command string
together with Farhelm's own flag template into a single HostText: let argv = HostText::from_host(format!("{command}
[--model <model>] [--effort <effort>]{tail}")); ... .child(HostSentence::new(id).mono(&argv)) validate_command
(crates/farhelm-proto/src/custom_agents.rs:57-70) allows non-ASCII characters including right-to-left scripts in
commands/paths. SPEC_impl.md, 'Host-supplied text', states: 'A host sentence (HostSentence) is one text element... with
each host value inside a left-to-right isolate (U+2066 … U+2069)... Host text is never concatenated with the app's words
in any other way.' Putting both command and the app's flags inside one HostText puts them in one bidi isolate so RTL
text in command can reorder the flags. How to verify: inspect custom_agents.rs:633-643 and host_text.rs:361-383. Fix:
keep command in its own HostText::from_host(&command) segment inside HostSentence and append the flags as a separate
segment (e.g., .text(...) with monospace inherited from the container, or a separate HostSentence part).

Reviewer metadata (g50_custom_agents_gui_settings_a/f7): suggested bucket: other (minor display/host-text-hygiene issue
in a local preview; control and bidi-override characters are still escaped by HostText::from_host and rejected by
validate_command).; confidence: confirmed (established via source and contract analysis; not executed).; possible cover:
none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/f7`.
