### What this was about

`farhelm agent hosts` is the command an agent runs to see the hosts in the fleet. When agents became able to ask for a
Tailscale host (`farhelm agent host add --tailscale`, shipped in 0.46.0), the listing's JSON form (`--json`) gained a
field saying whether each host is a Tailscale host, but the plain table, which is what an agent usually reads, did not.
So an agent told to check `farhelm agent hosts` after adding a Tailscale host could not tell from the table whether it
was one.

You decided the table gets a new last column, `TAILSCALE`, after `STATE`, reading `yes` or `no` on every row.

What was built: exactly that. Every row now ends in `yes` or `no`, whatever kind of host it is, spelled the way the
`farhelm agent agents` listing spells its yes/no columns. A Tailscale host still shows `ssh` as its kind. The JSON form
is unchanged. The instructions agents receive now point at the new column, SPEC.md and SPEC_impl.md say the table shows
it, there is a new changelog entry, and the TODO entry is removed.

### Things you should know

- **A script that read the last word of a row as the host's state now gets `yes` or `no`.** Anything reading the table
  by column position still finds `ID`, `NAME`, `KIND` and `STATE` where they were. The changelog entry's curation notes
  say the same.
- **`STATE` now gets the length limit other middle columns have.** In this table every column except `ID`, `NAME` and
  the last one is cut at 48 characters. `STATE` used to be the last column and so was never cut; now it can be. Host
  states are short fixed words (the longest, `unreachable-reprobing`, is 21 characters), so nothing is cut in practice.
- **A test fixture was updated too.** The fake agent the test suites use reads this table to check a host name. Its copy
  of the table was updated to include the new column. It reads only the name column, so its behavior is unchanged.
- **The changelog entry.** As the plan asked, this gets a changelog entry of its own (under Added) rather than an edit
  to the earlier Tailscale entry, because that change already shipped in 0.46.0. The plan's release note section asked
  for no highlight, and none was written.
- **Unrelated: a lint failure on main.** Running clippy with warnings as errors over the `farhelm` crate and everything
  it builds fails on main as it stands, before this change. The helm crate has a settings-saving function for ntfy
  notifications that only its tests call, so outside the tests it is flagged as unused. Nothing here touches it. The
  crates this change touches lint cleanly on their own.

### Open questions and possible follow-ups

- Possible follow-up: the unused helm function above makes clippy fail on main for anything that builds the helm. A
  one-line fix (remove it, or mark it test-only) would make that check usable again.

### The PRs

- #5161 `feat: show each host's Tailscale setting in the agent hosts table`: the new column, the agent instructions,
  SPEC.md, SPEC_impl.md, the test and fixture updates, the changelog entry and the TODO entry's removal.

### Checks

Run on the final change, whose base is still the latest main (main has gained only plan bookkeeping since).

- Through the test-run recorder, run 97eb4b63-de61-4a2d-bbd5-a3eca51ef49c (68 passed), one wording change to the agent
  instructions before the final tree:

  ```
  cargo nextest run -p farhelm -p farhelm-fixtures --bin farhelm --bin farhelm-fixtures --test agent_cli -E 'binary(agent_cli) | test(/^render::/) | test(/^agent_instructions::/) | test(the_hosts_listing_check_matches_a_whole_name_column)'
  ```

  That is every test of the agent command line, including the plain hosts table (pinned exactly, with one Tailscale host
  and one ordinary host) and the JSON form, the table code's own tests, the agent instructions' tests, and the fake
  agent's host-name check.
- Through the recorder on the final tree, run c9a3414a-9600-49a1-96da-9d4c18fc442f (61 passed), after that last wording
  change:

  ```
  cargo nextest run -p farhelm --bin farhelm --test agent_cli -E 'test(/^agent_instructions::/) | binary(agent_cli)'
  ```

  The table code and the fixture did not change between the two runs, so the first run still covers them.
- `cargo clippy -p farhelm -p farhelm-fixtures --all-targets --no-deps -- -D warnings`: clean. Without `--no-deps` it
  fails on the unrelated helm warning above.
- `cargo fmt`, `dprint check` on the changed specs and TODO.md, the changelog format check and the test-sleep check (no
  unannotated delays): all clean.

Earlier attempts: one recorder run (5362ac89-8033-446d-a9e5-fd7abcbc666f) never built, because the command named a
nonexistent binary. That was a mistake in the command, not a test result. An earlier passing run
(4b02ed42-4d44-4869-8e12-5b8607845071) is superseded by the runs above.

Skipped: the end-to-end suites, the workspace-wide run and the desktop app's tests. Only the command line's table output
and text changed; the one end-to-end test that reads this table checks only that the header names `NAME`, `KIND` and
`STATE`, which still holds. No earlier checks were reused.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It found no bugs and confirmed every acceptance criterion,
including that the JSON form is unchanged and that no state word is long enough to be cut now. Its findings:

- the changelog entry belonged under Added, not Changed;
- the fake agent's copy of the table no longer matched what the command prints, and named the wrong source file;
- the agent instruction line could name the column;
- a code comment overstated what the new cell guarantees.

All were fixed, and a verification pass by the same reviewer confirmed them. It also suggested one more wording tweak to
the instruction line, which was applied.
