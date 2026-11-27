---
kind: changed
---

`farhelm agent` and `farhelm spawn` now refuse an option whose value is missing, instead of taking the next flag as its
value: `farhelm agent host add dev --alias --tailscale` used to ask for a host named `--tailscale` without the Tailscale
option. A value that really starts with `-` is now joined to its option with `=` (`--title=-draft`), or given after `--`
when it is a plain argument such as a new title, a template name, or the command of `farhelm helm checkout-config
set-post-clone`.

Context for curation: this extends to every free-text option what v0.46.0 did for `--model` and `--effort`. It covers
the session, title, host, folder, recipe, template, prompt-file, idempotency-key, wrapper, command and resume-command
options and the name, title, host and checkout-config arguments. `--prompt` is the one exception and still takes a
message starting with `-` (a Markdown bullet, say) as a separate argument. `--prompt-file -` still reads stdin. Scripts
or agents that passed a hyphen-leading value as a separate argument need the `=` or `--` form; the instructions Farhelm
gives agents say so.
