# Desktop app idle CPU: where it goes, and how it grows with the fleet

NOTE: This is a point-in-time investigation, not a spec and not a plan. It records what the GPUI desktop app spent idle
CPU on at main `215b6fff5` (2026-11-26), how that was measured, and the fixes it suggested, ranked as they looked that
day. The maintainer's question was not "is the new app worse than the old one" but "does the current app behave
reasonably at idle, especially as the number of sessions grows", and what each improvement would cost in added
complexity (effort was explicitly not the concern). Two of the fixes below (the first two in the ranked list) were taken
up right away; the rest are recorded so they can be revisited.

## The setup

The desktop app ran on the shared test Mac: a Tart macOS 26 VM on an otherwise idle Mac mini, with Apple's
paravirtualized GPU (Metal sees a real, non-headless device and WindowServer submits work to it), a 1920x1080 point
display at scale 2 and 60 Hz, 12 virtual cores and 16 GB. System Integrity Protection is off there, so DTrace works.

The fleet ran on a separate Ubuntu 24.04 host with 8 CPUs, so that the Mac's own numbers are the app (helm included,
since it is embedded), the network, and nothing else. A 12-session fleet ran on the Mac's own local host instead, to give
a full visible list at a small fleet size.

Each session was a command launch of a small bash loop that redrew a spinner line once a second, which the supervisor
reads as Running. While a flag file existed on the host the loops printed nothing, which switched the whole fleet to
Idle without any helm involvement.

Optimized builds keep the helm's API credential in memory, so the fleet was seeded with the debug agent build
(`scripts/gui-agent.py`'s `helm.json`) and then measured with `scripts/devbuild-mac.sh --run-only` on the same state
directory. The sessions live in the host's supervisor, so they carry over. The measured build was the devbuild's release
profile plus line tables (`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`).

How it was measured:

- CPU as the difference in cumulative CPU time (`ps -o time=`) over a 60 second window, divided by wall time, for the
  app, its local supervisor and WindowServer. That is an exact average, unlike `ps`'s decaying %cpu.
- On-CPU user stacks with DTrace (`profile-997` filtered to the app's pid, 400-frame stacks), folded, demangled with
  `rustfilt`, and bucketed by the first Farhelm or GPUI frame that matched.
- Call rates with DTrace pid-provider entry probes on mangled names: GPUI's `Window::draw`, the session list's
  `render`, the row builder, and on the helm side the per-collection `ui_channel::diff` (13 calls per re-read) and
  `parse_control` (one per supervisor message). Probes on async functions count polls, not calls, which made an early
  count of `Channel::changes` meaningless; the synchronous `diff` is the reliable one.
- Wakeups with `sched:::on-cpu` aggregated by user stack, plus `top`'s context-switch counts.

"Front" means the app was frontmost; "background" means Finder was brought in front of it (the window still visible,
just inactive).

## The numbers

App CPU, percent of one core:

| Condition            | 12 sessions | 250 sessions | 500 sessions |
| -------------------- | ----------- | ------------ | ------------ |
| Running, front       | 4.8         | 6.6          | 7.85         |
| Running, background  | 2.0         | 3.2          | 4.3          |
| Idle, front          | 2.6         | 3.05         | about 3.7    |
| Idle, background     | 0.67        | 0.93         | 0.90         |
| Idle, app hidden     |             |              | 0.28         |

The supervisor stayed under 0.2% throughout. WindowServer added 0.3 to 1.6%, tracking how often the app drew. The
500-session idle-front number had not fully settled.

## Cause 1: every frame redraws the whole window, at a cost that does not depend on the fleet

A frame cost about 12.5 ms of main-thread CPU at 12, 250 and 500 sessions alike. The list's virtualization works: about
12 rows were built per frame at every fleet size. While any row pulses (a running session), the shared ticker drives
about 3.5 frames a second, which is about 4.4% CPU in front by itself. With an idle fleet the focused terminal's cursor
blink drives one to two frames a second. The frame time splits roughly evenly between Farhelm's render functions (the
session list's `render`, the session pane) and GPUI's own request_layout, taffy layout, prepaint and paint for the whole
tree, with `memmove` of large element builders showing up at about 13 to 15% of samples spread thinly across GPUI's
element construction.

The one part of a frame that grows with the fleet is the session list's per-frame pass over every listed session (the
listing, groups and order hold, and the `any_pulse`, `any_spinner` and `any_stamp` scans, the spinner one doing four
lookups per row). It went from about 1.75 ms per frame at 12 sessions to 2.6 at 250 and 3.0 at 500.

This is the same per-frame cost the earlier TODO entry "Idle CPU of the GPUI desktop app" describes for one session
(4.4 ms then); a full list of rows simply costs more per frame than one row.

## Cause 2: the helm recomputes everything, about once a second, for a running fleet

The maintainer's recollection was that the helm still sends the UI the whole session list on every change. It does not:
the UI channel pushes only the changed entries, each whole (plus the full order of session ids in every change). What it
does instead is recompute everything to find those changes. On each fleet feed bump, the channel re-runs all 13 pushed
collections' GET handlers through the in-process router (SQLite reads, a JSON decode of every cached session row, the
group, mute, seen and notification-mark lookups, serialization, a parse back into JSON values), compares the result
with a deep copy of what it last sent (`self.sent.get(..).cloned()` in `Channel::changes`), and records it again. Each
host refresh, in turn, decodes the supervisor's full session list and `replace_host_sessions` deletes and re-inserts
every cached row for that host, even though it already works out which rows changed.

The helm's share is what grows with the fleet. Running in the background it was about 0.35% CPU at 12 sessions, 1.4% at
250 and 2.75% at 500; in front, about 0.07%, 2.1% and 3.2%.

The rate is about one refresh and re-read cycle per second at 500 running sessions (13.1 `diff` calls a second, 2.3
supervisor messages a second), each cycle costing about 27 ms of helm CPU (about 54 µs per session). An experiment that
paced hint-driven host refreshes and the channel's feed-driven re-reads to at most one a second changed nothing (4.96%
running in the background against 4.29%, within noise), because the rate was already about one a second: the 200 ms
`SESSIONS_CHANGED_MIN_GAP` was never the limit.

What drives the cycles, from reading the supervisor's ticker (not isolated by an experiment): the ticker samples 16
sessions every 2 seconds, and each sample of a session that produced output moves its "last active" stamp when the
stamp is more than `ACTIVITY_STAMP_QUANTUM` (60 s) old. With 500 sessions each is sampled about every 62 seconds, so
nearly every sample moves a stamp, and every tick produces a change hint; the 3 second backstop poll then also finds
changes. The quantum's own doc comment reasons that the wake is per refresh rather than per session; that holds, but at
fleet scale per-refresh still means about once a second, for an age label shown in minutes.

## Cause 3: pure idle is mostly GPUI's display link

With nothing drawn at all (the window in the background, an idle fleet), DTrace's samples accounted for only about a
third of the CPU time `ps` reported. The rest was wakeups: about 186 on-CPU events a second, of which 60 were the
CVDisplayLink thread and 60 the main run loop it wakes, plus about 55 in the helm's tokio runtime. The vendored
`gpui-pre-macos` starts a display link per window and stops it only when the window's occlusion state says it is not
visible, so a visible but idle window, including one behind other windows, gets a callback per display refresh with
nothing to draw.

Hiding the app (which stops the display link) took the 500-session idle fleet from 0.90% to 0.28% and from about 186 to
17 wakeups a second. The remaining 0.28% is mostly the helm's 3 second backstop poll decoding the supervisor's 500-row
reply and rewriting all 500 cached rows (including the write-ahead log writes), with no change found.

## Fixes, ranked by improvement per unit of added complexity

1. Stop activity-stamp moves from triggering full cycles: have the supervisor move pending "last active" stamps
   together, on a fleet-wide schedule (for example once a minute), rather than per session as each is sampled, while
   status changes still hint at once. One function in the supervisor's ticker and no protocol change; the visible cost
   is that an age can lag by up to the quantum, which it already can. The expected effect (not measured) is that a
   running fleet costs about what an idle one does: roughly 4.3% down to about 1% for 500 running sessions in the
   background, and the helm's share in front from about 3.2% to a fraction of a percent. It also removes most of the
   background redraws those pushes cause. Hosts keep the old behavior until their supervisor is updated.
2. Small helm cleanups, each low complexity: write only the cached session rows that changed in
   `replace_host_sessions` (it already computes which); stop deep-copying the last-sent state in `Channel::changes`; and,
   optionally, a slower backstop poll now that change hints exist, which trades how soon a missed hint is noticed and is
   the maintainer's call.
3. Pause the display link when there is nothing to draw and restart it when something invalidates the window. This
   removes about two thirds of pure-idle CPU whatever the fleet size. Medium complexity: a patch to the vendored GPUI
   that has to be carried, with a real risk of a window that fails to redraw when it should.
4. Fewer frames in front: the running pulse and the cursor blink each redraw the whole window. Slowing or dropping the
   pulse, or stopping the blink after the terminal has been quiet for a while, adds no code complexity but is a UX
   decision. Front-window drawing cost scales directly with this frame rate.
5. Cheaper frames: caching views, the lever the "Idle CPU of the GPUI desktop app" TODO entry already names, which needs
   a GPUI patch to replay cached views' accessibility information. The largest effect on front-window cost, and the most
   complexity.
6. Not recommended at the time:
   - making the session list's per-frame pass incremental (about 0.45% at 500 running sessions in front, with many
     inputs to invalidate correctly: selection, marks, the order hold, collapse state, drags);
   - an in-memory decoded session cache in the helm, or skipping unchanged collections with per-collection revisions,
     both of which work against SPEC_impl.md's deliberate "re-run the GET handlers and diff" design ("What is pushed,
     and how changes are found");
   - delta or "unchanged since" replies from the supervisor's session listing, which needs a protocol change and old
     hosts handled.

The maintainer chose to do 1 and 2 at once.

## Caveats

These are VM numbers on paravirtualized hardware, percent of one virtual core; they compare conditions on one machine
and are not comparable to the older figures recorded on the physical test Mac. The frame-rate counts and the CPU windows
were separate intervals, which is why idle-front frames briefly looked more expensive than running ones; treating every
frame as about 11 to 14 ms fits all the data. The cause of the running fleet's cycle rate is inferred from code and the
measured rates, not isolated by changing the supervisor.
