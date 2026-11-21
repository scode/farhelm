# Reopening a session can restore a shell the supervisor cannot attach

Reviewed commit: 9864b874b3ac36670cb52a566ecbc08610a71430

## TLDR

Reopening a session can leave it on a remembered shell tab that cannot attach, even while the main terminal is
available.

## Details

**high**. Confirmed by source and contract inspection.

Confirmed from source. With the primary terminal usable, reopening can show a remembered shell's reconnecting pane until
the user selects the primary terminal or the supervisor restarts. Source specifically supports a persistent refusal:
supervisor service/handlers.rs:1552–1577 distinguishes re-adoptable terminal hosts from an unreadable host it will
refuse until supervisor restart. Such a tab can remain listed with its id alone
(service/core/host_tabs.rs:168–187,203–223,266–276), and the supervisor refuses that individual tab before takeover
(service/terminals.rs:241–247. Service/handlers.rs:1693–1704). Both kinds of typed Unavailable refusal reach the helm
client as `Other` (helm terminal.rs:311–315,501–520), so it retries and can probe indefinitely (helm-client
terminal.rs:383–387,451–482. SPEC_impl.md:549–559). TerminalSet selects on entity construction before attach and retains
it on stable membership (set.rs:539–545,563–602), while stream state stays in the attached terminal
(attached.rs:1223–1291). With a stale list entry for a truly closed shell, authoritative NotFound instead becomes
TabClosed (supervisor service/terminals.rs:210–247. Helm terminal.rs:291–315,501–520). The client stops automatic
retries and the never-attached view has no Reconnect (helm-client terminal.rs:383–387,483–498. Attached.rs:847–865), but
the pane still awaits the roster removal before switching. That bounded interval is separate from the persistently
listed unreadable-host case. Ordinary progress, transient transport reconnect after a live attach, and a viewer's
explicit Stalled code are outside the claim (tabs/f9. SPEC.md:3287–3299). SPEC.md:2250–2253 promises primary-terminal
fallback when a remembered tab is gone or unavailable to attach. Possible_cover: SPEC_impl.md:482–486 describes local
construction under the cap, without expressly accepting a failed initial restore staying selected. Estimate high for the
indefinitely unusable default despite a working primary terminal. Confirmed source route, no runtime observations.
A315f83db44cf741961ff7ed79712b92755fd0f1 changes comments only. Pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f
retains the decision and stream paths. 05ae73f23eb2cbb7ab284ca42473d66a771207a9 changes the primary label for shell
sessions, without changing the chosen=None fallback. Test coverage: set/tests.rs:718–746 exercises the local attach cap,
:875–891 removes the tab from the roster before reopening, and :633–683 stalls streams only after they are live. These
cases do not exercise an initial per-tab refusal with an unchanged roster and a working primary terminal.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tabs/f1`.
