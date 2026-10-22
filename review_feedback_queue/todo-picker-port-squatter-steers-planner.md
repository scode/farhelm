# Another local account can take the TODO picker's fixed address and get its text pasted to the planning agent

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

The page the maintainer uses to pick TODO entries for planning always lives at the same local address, and the
maintainer is expected to keep that address open in a browser. While no picker is running, another account on the same
machine can serve its own lookalike page there. If the maintainer uses that tab, the page's Copy button can put any text
on the clipboard. The planning agent acts on the pasted text without confirmation and treats everything pasted with it
as the maintainer's own words, so the other account can steer an agent that runs shell commands as the maintainer.

## Details

Found by the 2026-10-22 prioritized persistent review, group `221-maint-scripts-9of9` (finding f2). The reviewer
proposed dropping it. An independent check disagreed and asked for it to be recorded, for the coverage reasons below.

Mechanism:

- `scripts/todo-picker.py:35-39` (module docstring): the port is fixed at 14003 "because the maintainer asked for a
  fixed, memorable URL to keep open in a browser". `plans/.agents/planning.md:128-131` reuses that port for the up-front
  questions page and says "the maintainer keeps that one URL open in a browser".
- `serve` (`scripts/todo-picker.py:413-440`) binds `127.0.0.1` and refuses when the port is taken (`:432-435`). The
  script never serves another account's content itself. But the flow (`plans/.agents/planning.md:25-29`) then serves on
  the next free port and gives the maintainer that URL, while the tab kept open at 14003 now reaches whatever holds the
  port. The page has no per-run secret or identity in its URL, so the maintainer cannot tell another account's page from
  the picker. TODO.md is in the repository, so a lookalike can show the real bucket.
- `plans/.agents/planning.md:30-33`: the pasted line "is that request, so do not ask for confirmation; anything the
  maintainer writes alongside it is what they said about the entries." The Copy handler is page JavaScript (`copy()` in
  the page script, around `scripts/todo-picker.py:333-345`), so a lookalike can copy text that differs from what it
  shows. Agent terminals often show multi-line pastes collapsed, so extra lines need not be visible when pasted.

Scenario: another account binds `127.0.0.1:14003` while no picker or questions page is running. The next todo planning
flow finds the port taken and serves on 14004. The maintainer reloads the tab they keep open, as the docstring intends,
uses the other account's page, clicks Copy, and pastes into the planning session. That session is an interactive agent
with shell access under the maintainer's account, and it receives the injected text as the maintainer's message.

How to verify: as a second account, run a static server on `127.0.0.1:14003`, then start the flow and note that the
picker moves to 14004 while the 14003 tab serves the other page.

Fix sketch: put an unguessable per-run token in the URL path and serve 404 without it, so a tab kept open at the bare
port never looks valid. Or refuse to fall back to another port, and tell the maintainer that something else holds 14003.
Either costs a little of the "one fixed URL" convenience; a per-run path could be printed each time.

Coverage, as judged by the independent check:

- SPEC_impl.md "Development tooling assumes a single-user machine" (`SPEC_impl.md:4923-4924`) names four kinds of tool:
  "The test harnesses, the capture and recording scripts, deflake and the docs preview". The picker is none of them, and
  `scripts/todo-picker.py:35-36` says so itself ("it is not a test harness").
- `TRIAGE_OUTCOMES.md` "notification-topic-leaks-through-curls-process-arguments.md" accepted a planning-system tool
  under that decision, but for a secret in process arguments, which the spec names. Text that the planning agent is told
  to act on without confirmation, served on a squatted port, is a different consequence.
- The only port-squatter acceptance, SPEC.md "Security" (Client to helm: "That gap is accepted in v1: the browser UI is
  recommended only on a machine with no other, untrusted local users"), is about the helm's browser UI.

Suggested bucket: recorded under other, per the independent check, which said other, or high if triage counts steering
the planning agent as serious. The original reviewer said high, possibly highest. The bucket is for triage to settle.

Confidence: likely. Unverified premises: that the maintainer would use the tab kept open at 14003 after the agent names
a different port, and that the paste is not read before sending.

Possible cover: SPEC.md "Security" (Client to helm, the accepted port-squatter gap) by analogy only; SPEC_impl.md
"Development tooling assumes a single-user machine" does not name this tool.
