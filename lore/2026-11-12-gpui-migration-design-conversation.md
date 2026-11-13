# GPUI migration design conversation

NOTE: Historical design and planning conversation recorded on 2026-11-12, preserved as discussed and not maintained.
Proposals, corrections, and decisions appear in conversation order; later replies revise earlier suggestions. This
records the discussion rather than specifying current product behavior or starting migration work.

The transcript below preserves the supplied conversation, including terminal formatting and typos. The local checkout
path, private prototype repository identifier, and notification topic are redacted for publication.

```text
 ▐▛███▛█   Claude Code v2.1.323
▝▜██████▀  Opus 5.5 · API Usage Billing
 ▝▝   ▝▝   [local checkout]


❯ Alright I want to talk about actually moving all of farhelm over to gpui.     
  Please see [private GPUI prototype repository] on github (intentionally private) for a
  prototype and some test results.                                              
                                                                                
  I want to assess how to do this best. From two perspectives:                  
                                                                                
  - How to get to a good state and avoid poor practices (like how we ended up   
  with a bunch of unstructured random focus issues in farhelm today, and only   
  recently started fixing it and committed some actual rules). We want to       
  follow reasonable modern UI practices not flail around and let things grow    
  complex in an unstructured way.                                               
  - At the same time, finding some way of being incremental. Either incremental 
  in the work itself (checkpoints, testing things early, etc etc), or possibly  
  shipping releases partially migrated. I'm not sure if the latter is worth     
  it.                                                                           
                                                                                
  I woudl not do this using the farhelm planning system. I'd want to do it with 
  something like an orchestrating agent who over-sees the whoel thing, and      
  then sub agents with various areas of responsibilitiy churning away at pieces 
  in parallel.                                                                  
                                                                                
  We want appropriate ahead of time planning, and good checkpoints and          
  stoppoing points for assessing "is ths code good? are we growing needless     
  complexity? do we need to re-think something along the way?".                 
                                                                                
  What are your thoughts.                                                       

● Skill(skillette)
  ⎿  Successfully loaded skill

  Read 2 files, ran 3 shell commands (ctrl+o to expand)

● Reading the prototype's report and status docs before forming a view; next is
  STATUS.md (the "read it first" doc) and the rest of the report.

  Searched for 2 patterns, read 4 files, ran 2 shell commands (ctrl+o to expand)

● Read the prototype report, status, entity design and brief; now skimming
  Farhelm's new focus rulebook and the UI's size so the advice fits what exists.

  Ran 2 shell commands (ctrl+o to expand)

● I read the prototype's STATUS, REPORT, entity-design doc and brief, plus
  Farhelm's new Keyboard focus section in SPEC.md. My main view is that the
  focus mess and the prototype's stale-screen bugs have the same cause. Both are
  app-wide rules that each feature decided for itself, and you avoid repeating
  that by building a few mechanisms once, before any parallel work starts, and
  making it hard to go around them.

  Close the open gates first, or alongside

  STATUS.md still lists four gates as unevaluated or failed: speed compared with
  the current app (Mac part 12 was skipped), the whole Linux desktop handoff
  including real GPUs, terminal input-method composition on the Mac, and screen
  readers. Each of the first three could end the GPUI route, or the Linux half
  of it. Running them costs days; the migration costs far more. I'd run them
  during the foundations phase below and not split work out to parallel agents
  until they pass.

  Two product decisions also need to be settled first: whether dropping the
  browser UI is final (brief section 9), and whether the SPEC sections on
  topology and "one GUI at a time" get rewritten up front, not at cutover.

  Avoiding unstructured growth: foundations before features

  The prototype's report shows the pattern. Six stale-surface bugs, all silent,
  none caught by the authoring agent's own tests, and every surface carrying its
  own guards and notify calls. That is the focus story again, with a different
  symptom. The fix is a short list of app-wide mechanisms, each with one owner,
  each made hard to bypass:

  1. State and updates. No view keeps its own copy of the data with a setter. A
     view is either uncached, reading the store when it draws (always correct),
     or sits behind one generic cache wrapper. That wrapper computes the view's
     inputs from the store on every change and redraws only when they differ.
     Built that way, a forgotten notify can't happen, so you don't depend on
     reviewers spotting it. This is my design idea, not something the prototype
     tested. It's the first thing to check in the foundations phase, including
     what it costs at 200 rows.
  2. Focus. The rulebook already exists and is written in product terms. Build
     it as one focus module that owns:
     - where focus goes when nothing holds it (the selected session's terminal),
     - a counter of user input, so a result that arrives later can tell whether
       the user moved on,
     - the dialog stack, with its focus trap and restore,
     - a record of who opened each dialog or menu, so nothing closes what it
       didn't open.

     A lint forbids moving focus anywhere else in the code.
  3. One layer stack for dialogs, menus, popovers, tooltips and approval cards,
     plus the rule that nothing may cover a terminal link's disclosure. The
     prototype grew separate pieces for each of these.
  4. One table for key routing: app shortcuts versus the terminal, and which
     context wins. The prototype's Ctrl+C regression came from this.
  5. One pattern for requests that finish later: pending state, retries with the
     same request key, and the "has the user moved on" check from the focus
     module.
  6. Our own component layer over GPUI Kit. Each component requires a label,
     hover help and an accessibility role in its constructor, so the
     tooltip-coverage test becomes something the compiler checks. It also keeps
     Kit's weekly API changes in one place, and a lint forbids using Kit
     directly anywhere else.

  Write these up as a GUI architecture section in SPEC_impl.md, as rules in the
  style of the focus rulebook, plus a short GPUI cookbook built from the
  prototype's friction log. Every agent working on an area gets both documents.

  Then build two or three thin features end to end on top of them: session list,
  row menu, delete confirmation, terminal attach. That is the most important 
  checkpoint. You review the code, not a report about it, before anything runs
  in parallel.

  What to reuse

  - Move into Farhelm: the prototype's three framework-free crates (terminal
    grid model, helm client, key and mouse input mapping) and, after a review,
    most of its terminal (about 6,200 lines, its most mature part).
  - Reference only: the prototype's sidebar, menus and dialogs. They were built
    to answer questions and carry measurement modes and experiment switches.
  - Can start on main today: move the framework-free policy modules out of the
    current UI crate (reconnect, attachments, status, rows, launch sequence)
    into a shared crate. That shrinks the rewrite and has value on its own.

  How to be incremental

  - Mixing GUIs in one app: no. A web view inside a GPUI window means two focus
    and keyboard systems in one window, the exact problem you're trying to get
    rid of.
  - Shipping the new app alongside the old one: yes. It's cheap because the
    helm's API is already the boundary. Put the new app in dev and RC releases
    as a separate opt-in app from the point it covers your daily loop. On Linux
    it's simply the first desktop app, so nothing regresses there. One catch: to
    dogfood it, it has to run on your real state in place of the current
    desktop app, not next to it. The prototype deliberately refuses to do that
    today.
  - Freeze the old UI except for critical fixes. You're already dropping its
    focus TODOs. Switch the Mac stable release over in a single release.

  Milestones, each one a stopping point:

  1. M0: gates passed, product decisions made.
  2. M1: foundations plus the end-to-end slice. Your review of the code.
  3. M2: covers your daily loop. Dogfood it for some days, then an architecture
     health review.
  4. M3: parallel work by area until the new app does everything the old one
     does. A checkpoint per area.
  5. M4: cutover: remove the web UI, the WebAssembly build and Playwright,
     rewrite the spec, package for Linux.

  Orchestration

  - Ownership. One agent (or the orchestrator) owns the foundations. Area agents
    can't change them; they file requests, and the orchestrator decides whether
    a request is a real gap or one area working around the design. That rule
    does more than anything else to stop each agent inventing its own pattern.
  - Areas: terminal (moving it over), sidebar and host rail, launcher and
    templates, settings and hosts, the other dialogs, desktop integration and
    Linux packaging, approvals and agent-spawned sessions. Each gets its SPEC
    sections, the old code as a behavior reference, and its Playwright specs as
    a list of acceptance cases. Port the behavior, not the spec files: most of
    it fits the fast in-process tests, with a smaller end-to-end set run through
    the automation bridge.
  - Review on every change by a fresh-context reviewer with a fixed checklist:
    did any view keep its own copy of data, does focus go through the focus
    module, any direct Kit use, are labels and hover help present.
  - Architecture review at each milestone by a reviewer who reads the whole tree
    for duplication and drift. Track numbers: lines per area compared with the
    old UI, lint exceptions, special cases added to the foundations, cache
    wrappers.
  - Stop and rethink when an area needs to go around a foundation, a foundation
    gains a special case for one area, or the same kind of bug keeps coming
    back.
  - Upgrade GPUI and Kit only at milestones, never during parallel work.

  If you want ready-made machinery, the scode-powerhorse skill already handles
  "large parallel chunks on strong delegates while I gate and integrate". For a
  run of this length, a goal file with a working log fits better than the
  planning system.

  One pushback

  Your TODO cleanup commit puts the rewrite "days to a couple of weeks out". The
  parallel phase is cheap with agents; the foundations phase is where the
  code's quality gets decided. I'd give it real time and your attention, and not
  let the start date push the parallel work forward early.

  If it helps, I can draft the GUI architecture section for SPEC_impl.md next;
  that's the document every later step depends on. The prototype clone is in my
  scratch directory if you want me to dig into a specific part.

✻ Worked for 2m 18s · done 6:19 PM

※ recap: We're working out how to move Farhelm to GPUI: close the remaining go/no-go checks first, build shared state and focus foundations before parallel work, and ship the new app alongside the old one. Next step: your go-ahead for me to draft the GUI architecture rules. (disable recaps in /config)

❯ dont get distracted by the gates. gates were intenveted, it snot a law of     
  physics. for example  i don't care that much about IME. don't treat what you  
  read as some kind of law. it's data. not somethign we must take for sacred.   
  for speed, we can plan to de-risk that by doing anything early that is a real 
  serious risk for speed. gpui stuff is well understood. zed uses it for one,   
  being very known for its speed. if there are areas where we are concerned     
  with speed aside from "oops we fucked our implementatin up" (fixable),        
  perhaps with the terminal stuff, let's de-risk that by proving it out early.  
                                                                                
  and yes a large part of the point is to drop the entire browser mess, and yes 
  let's rip the band aid off of "one GUI at a time" while we're at it.          
                                                                                
  1. agree                                                                      
  2. agree. we jrecently added those focus rules, we should keep them (unless   
  we find a good reason they are wrong/bad). i intentionally went through all   
  of the them with the agent wich mockups etc                                   
  3. (probably fine, not sure what you mean I think you mean z order visually,  
  in which case it makes sense)                                                 
  4. sure                                                                       
  5. agree                                                                      
  6. yes for sure, i was going to suggest this, glad to see it in your list. we 
  don't want to ad-hoc this (and btw we must NOT accidentally pull in random    
  third party stuff that's GPLed or similar, we need to be license cautious; i  
  say this because i think there are community components that are GPL).        
                                                                                
  I would also sgugest trolling through past bugs in farhelm out of the commit  
  history. I'm sure there are other patterns besides the focus issues for us to 
  consider here.                                                                
                                                                                
  And no, i do NOT want to ship a parallel app. lThis is a hard no. Agents keep 
  suggesting it. It does not make sense in the real world situation here for    
  farhelm.                                                                      
                                                                                
  But yes my plan is to freeze farhelm except for critical issues until we get  
  to through the migration. We'll just fix forward on main branch. If I need to 
  land a critical fix I'll cut a special fix release that isn't from main. I'm  
  freezing the whole thing not just the UI.                                     
                                                                                
  For the workflow you outline, sure. But also, some explicit review for "are   
  we adding complexity that isn't necessary" must be done as well because       
  otherwise things tend to spiral out of control. And all the agents need to    
  know that there is an out - product/SPEC/judgement questions can be surfaced  
  (to me, via you the orchestrator). Agent should not take some  little         
  detailed interpretation fo somethign and invent some 10k complicated          
  insanity; if in doubt about intent when something is costing a lot of         
  complexity, surface it for asseessment while trying not to block progress if  
  other work can proceed while it's resolved.                                   
                                                                                
  For goal stuff, scode-build-goal is the thing I've been using very            
  successfully (please understand its review system and such). But, it's also   
  pretty sequential.                                                            

  Read 2 files, listed 1 directory, ran 1 shell command (ctrl+o to expand)

❯ also dont over index on what the prototype ran into, do some research as well 
  on common pitfalls peopoe run into                                            

❯ with GUIs in generalk with GPUI                                               

❯ see if you can surface more foundational pieces we need to have in place.     

  Ran 1 shell command (ctrl+o to expand)

● I'm launching three read-only research agents in parallel to look into
  Farhelm's bug history, GPUI-specific pitfalls, and general desktop GUI
  pitfalls. · summary

● 3 background agents launched (↓ to manage)
   ├ Farhelm UI bug archaeology
   ├ Research GPUI pitfalls
   └ Research general GUI architecture pitfalls

❯ i dont' want to over engineer and build things we don't need to be clear, we  
  can also have things on a "watch list", to see if it ends up being needed.    

❯ the main thing is not sitting around for days with agens churning out         
  incredible complexity that isn't needed                                       

❯ oh and agent driven and automated testing MUST be available very early and    
  used as work happens.                                                         

● The research agents are running: Farhelm's bug history, GPUI-specific
  pitfalls, and general GUI pitfalls. I'll fold their findings in when they
  finish. In the meantime, here's where I am on your points.

  Speed. Agreed: the gates were a way to frame the prototype, not rules. Two
  speed risks seem real to me, as opposed to ordinary implementation mistakes:
  - The terminal under a flood, on real GPUs. That covers text shaping with CJK
    and emoji fallback, and several terminals attached at once. The Mac run's
    2–5 frames a second during the 200-session flood was never explained.
  - The session list when most rows are animating. A pulse tick touches nearly
    every row, so caching rows doesn't help.

  Both get proved out early, on the Mac and on one real Linux GPU, against the
  current app. Zed's own terminal under the same flood is a useful reference for
  what GPUI can reach. Everything else I'd treat as ordinary engineering.

  Layers. Yes, that's z-order, plus what goes with it: which layer gets the
  clicks, what's inert underneath, which layer an Escape closes, and the rule
  that nothing may cover a terminal link's disclosure. One owner for all of it.

  Licenses. I'll enforce this mechanically:
  - Add a cargo-deny license allowlist. The repo has no deny.toml today.
  - Agents don't read the source of Zed's GPL crates (ui, theme, workspace,
    terminal_view and others), because an agent that has read code can reproduce
    it.
  - GPUI itself and GPUI Kit are Apache-2.0. One research agent is listing the
    GPL components in the community ecosystem.

  No parallel app. Understood; I won't raise it again. With the whole project
  frozen and no releases from main, main can be in an unshippable state for a
  while. Incremental then means checkpoints in the work, not releases. I'd also
  delete the web UI, the WebAssembly build and Playwright early, not at the end.
  Agents then can't copy Dioxus patterns or keep old code compiling, and they
  read the frozen state from a reference checkout instead.

  One GUI at a time. My reading: with no browser and one window per machine, the
  concept goes away. The only concurrent writers left are the command line and
  agents, and their existing rules ("a GUI write must not undo an agent's
  change") stay. Tell me if you meant something else.

  Keeping complexity down. You're asking for three things, and I'd make each one
  concrete:
  - A watch list. It's a file where each entry is a mechanism someone thought
    might be needed, why, and the trigger that would justify building it. An
    agent that's tempted to build something speculative writes an entry there
    instead. The foundations themselves should be judged against it too:
    anything not backed by a real bug class or a SPEC rule starts on the watch
    list.
  - Size budgets, to catch the "days of churn" problem. Each unit of work gets
    an expected size, using the prototype or the old UI as the comparison: the
    old UI's version of the same area, or the prototype's row menu at about
    1,000 lines. At roughly twice the expected size, the agent stops that piece
    and reports it. The point is to stop early and ask, not to come back with a
    10k-line answer to a detail of the SPEC.
  - Complexity review at two levels. Per change, the reviewer gets an explicit
    question: what here is more than the requirement needs? Per milestone, a
    fresh reviewer reads the whole GUI tree with scode-build-goal's planning
    charter. That charter already asks the right question ("where this plan does
    more than the user needs… propose the smallest coherent design").

  The way out for agents. Every brief says: when a SPEC detail or product
  question is driving a lot of complexity, park that piece, write the question
  to the shared question file, and keep doing other work. I answer what the
  settled decisions already cover and batch the rest to you. scode-build-goal's
  unattended fallback already works this way ("pause only dependent work"), so
  the briefs extend it rather than invent something new.

  Testing early. Agreed, and it goes first, before any feature. The first
  milestone delivers:
  - in-process tests,
  - headless screenshots,
  - the automation bridge driving the real app against a real helm and
    supervisor, so an agent can operate the running app from a structured
    snapshot of the UI.

  Every area agent then checks its own work through the bridge as it goes, and
  so do the reviewers. A feature isn't done until an agent has driven it in the
  running app. The prototype showed the bridge working on Linux. On the Mac,
  snapshot, click and key presses worked against a real window; typing and
  reading terminal text weren't tried, so that's an early item.

  Workflow and scode-build-goal. I've now read it and powerhorse. What I like
  and would keep:
  - the up-front questions,
  - the planning review against unnecessary complexity,
  - scope reassessment before a big departure from the plan,
  - the per-PR review gate,
  - decision logging,
  - the self-resuming working log.

  The sequential limit applies within one goal. Here's the shape I'd propose:
  - Foundations and testing as one goal, run sequentially. That's right for
    design work, and it ends at your code review.
  - Then one goal per area, each written with scode-build-goal and run as its
    own session in its own sibling checkout, several at once. Each area stays
    sequential inside, with the proven per-PR machinery.
  - An orchestrator session above them. It owns the foundations (area goals
    can't change them; they file requests), the question file, the watch list,
    landing on main and the cross-area complexity reviews.

  One decision for you: the goal skill's done criterion is "open PRs, you
  merge". With several areas running at once, their stacks drift apart unless
  they land on main often. Since main is frozen anyway, I'd suggest letting the
  orchestrator merge PRs once they pass review, with your review at each
  milestone.

  I'll put together the foundations list once the three reports are in.

✻ Churned for 2m 49s · done 6:38 PM

❯ so the actual GUI can be driven by agents, observed, tested etc etc           

❯ in addition to replacements for all the playwright deterministic tests        

❯ and agents must be able to take sreenshots                                    

❯ so i can e.g. get the latest batch of screenshots from you and take a look    
  and sanity check things look visually fine, and you and your agents can       
  actually test things end to end for functional stuff                          

❯ ok now i'm reading what youjust told me                                       

● Agreed, and it fits the plan. Testing needs four separate things, all in place
  in the first milestone:

  1. Agents drive and watch the real app. That means the automation bridge in
     debug builds, against a real helm and supervisor with fake agents. Agents
     can read a structured snapshot of what's on screen, click, type, press
     keys, wait for conditions and read terminal text. That's how agents test
     end to end while they work, and how reviewers check a change.
  2. Deterministic replacements for the Playwright tests. Most become fast
     in-process tests, and a smaller set runs end to end through the bridge.
     Behavior is ported from the SPEC and the old specs' acceptance cases, not
     one spec file at a time.
  3. Agents can take screenshots. There are two kinds: real-window captures
     through the bridge, which show the real look on the Mac and Wayland, and
     deterministic headless captures compared against reference images. The
     prototype had both on Linux. On the Mac, headless captures work; a
     real-window screenshot worked once in the Mac run but needs proving as a
     routine step.
  4. A screenshot batch for you. A script captures a fixed set of scenes from
     the current tree: main surfaces, dialogs, menus, light and dark, narrow and
     wide windows, plus whatever changed. I can hand you the latest batch any
     time as a single browsable page, so you can check the look without running
     anything.

  The research is still running. I'll come back with the foundations list when
  it's done.

✻ Brewed for 9s · done 6:38 PM

❯ regarding agents reading source: dedicated separate isolated agents that look 
  for approaches that work are okay, if they deliver back information about     
  generla approaches without source code details.                               
                                                                                
  for GUI: note that we DO still want to keep the "helm" concept, make sure we  
  aren't inherently deleting that. we may still support running a remote helm.  
  the simplification is "only one UI at a time", not removal of the helm/UI     
  separation.                                                                   
                                                                                
  size budgets: its fine if size budgets are used to trigger a "hey wait a      
  minute, let's have an independent review for simplicity, alternate approaches 
  etc", and not just a "omg i hit a magic number now i'm blocked". (surface ot  
  me if necessary of course)                                                    
                                                                                
  agree aout what to keep from scode-build-goal                                 
                                                                                
  for the sequential/concurrent and how to run: I think that's ok but can you   
  elaborate how you're gonna set up the comms between the individual goal       
  executors and the orchestrator                                                
                                                                                
  for PRs on main: yes, for this work we can merge on main and use main as the  
  "open development branch" rather than "fully ready trunk". that's okay.       

● The pieces below are files and messages; I'd start with a small version of
  this setup and grow it only if it turns out to be needed.

  Source reading. Understood: an isolated agent may read code, GPL or not, to
  learn which approaches work, and reports back only the general approach, never
  code. The GPUI research agent running now is briefed roughly that way.

  The helm stays. Corrected. The GUI talks to the helm only through its HTTP and
  WebSocket API, as the prototype did, and never calls into helm internals even
  when the helm runs in the same process. That keeps a remote helm possible
  later, and I'll make it a foundation rule with a lint. The simplification is
  one UI per helm, not dropping the split. One question: do you want that
  enforced, so a second UI takes over from the first or is refused, or does it
  stay best effort as SPEC.md has it today?

  Size budgets. Agreed. Going over a budget triggers an independent review of
  whether there's a simpler approach, and it reaches you only if that review
  can't settle it. Work doesn't stop.

  Merging on main. Noted: during the migration main is the open development
  branch.

  Communication between goal executors and the orchestrator

  Files hold the state; messages only wake someone. Sessions compact, restart
  and resume, so nothing important can live only in a message. A shared
  coordination directory sits beside the checkouts, next to the goal files:

  farhelm-gpui-coord/
    PLAN.md              orchestrator: milestones, areas, status
    decisions.md         orchestrator: settled answers, append-only
    watch-list.md        orchestrator: deferred mechanisms + their triggers
    areas/<area>/
      outbox.md          executor → orchestrator, append-only
      inbox.md           orchestrator → executor, append-only

  Each file has exactly one writer, so nobody needs locks and nothing gets
  overwritten. Entries carry an id and a status, so either side can cite one
  ("re Q-terminal-7").

  What an executor writes to its outbox:
  - a question about the SPEC or the product: what it blocks and what the
    executor does in the meantime,
  - a request for a change to the foundations (executors don't change the
    foundations themselves),
  - a proposed watch-list entry,
  - a size-budget trigger with its review's findings,
  - one status line per PR when it lands.

  What the orchestrator writes to an inbox: answers, directives such as "pause
  at the next PR boundary" or "resume", and notices such as "foundation X
  changed, rebase and read the note".

  Wake-ups. Claude Code sessions on this machine can message each other, so
  whoever writes an entry that needs attention also sends the other side a short
  ping. As a fallback, the orchestrator scans the outboxes every 20–30 minutes,
  and executors read their inbox at session start, at each resume and at each
  PR boundary. Codex executors can't receive pings and just use the boundary
  checks, which is fine because nothing is designed to wait on a ping.

  Questions. I answer whatever the settled decisions already cover, record the
  answer in decisions.md, and reply in the inbox. Real product or SPEC questions
  go to you in batches in my session, with the tradeoff stated and what's
  waiting on each one. If you want a push notification when a batch is waiting,
  I can send one with ntfy. The executor keeps working on other things in the
  meantime.

  Landing. I'd let each executor merge its own PR once its review gate and the
  checks pass, so I'm not a bottleneck. Mechanical checks keep that safe:
  - a PR from an area may not touch the foundation paths,
  - the license allowlist,
  - the GUI rule lints: focus only through the focus module, no direct GPUI Kit
    use, the GUI talks to the helm only through its API.

  I review what landed after the fact, on a schedule, and fixes go forward on
  main. When main moves under something an executor depends on, I put a notice
  in its inbox.

  Milestone stops. I post "pause at the next PR boundary" to every inbox.
  Executors finish the PR they're on, log their state and stop. Then a fresh
  reviewer runs a complexity review across the whole GUI tree, you look at the
  code and a screenshot batch, and I post "resume", or revise the goal files
  first if something needs rethinking.

  Starting executors. I'd suggest you start each one yourself with /goal <file>
  in its own checkout, the same as your current scode-build-goal runs, so you
  can look in on any of them and they resume the way you're used to. I could
  launch them headless instead, but long runs that cross many sessions are
  easier to resume when you start them.

  When the three research reports are in, I'll send the foundations list with
  the watch list split out.

✻ Crunched for 48s · done 6:46 PM
● Agent "Research general GUI architecture pitfalls" finished · 10m 31s

● The general GUI research is back. The other two reports, on Farhelm's bug
  history and on GPUI pitfalls, are still running. I'll send the combined
  foundations list, with the watch list split out, once all three are in.

✻ Brewed for 4s · done 6:48 PM

● Agent "Farhelm UI bug archaeology" finished · 12m 25s

● The bug-history report is back too; that leaves the GPUI pitfalls research.
  Two things in the bug-history report need correcting before they go into the
  plan:

  - It suggests calling the helm in-process, dropping the HTTP/WebSocket link,
    so that link can be treated as one that never fails. That conflicts with
    what you said: keep the helm/UI split, and possibly a remote helm later.
    I'll keep the API boundary.
  - It says GPUI has no accessibility tree. The prototype contradicts that: it
    set roles and labels in GPUI's accessibility tree. The other research found
    the GPUI change that added it.

  The rest is strong. It classified 386 GUI fix commits into about 19 bug
  classes, and several matter more than focus did. I'll combine everything when
  the GPUI research finishes.

✻ Worked for 8s · done 6:49 PM

❯ single helm: given the chnages we are making here anyway, yes let's actually  
  enforce once-helm-at-a-time unless it turns complicated in which case we can  
  drop it for later as long as we continue to write code assuming one ui at a   
  time and don't add complexity to try to deal with concurrent uis.             
                                                                                
  (btw: there are a couple of plans in flight as we speak, part of tmux         
  removal, but it should have very little interact witn GUIs. i'm just letting  
  you know, so SOME change to main will happen before we actually fully start,  
  just be aware, you can see the plans in the planning system right now)        
                                                                                
  coordinator comms etc - sounds good. for this i expect to use claude only, no 
  codex so you can assume claude code for all agents working on this. (this     
  could chnage if this takes long enough time but we can adapt if that happens  
  later)                                                                        
                                                                                
  yes please use ntfy on [notification topic] (normal urgency not high etc,
  unles sI ask for temporary periods of time). in general i may be away for     
  hours at a time at night or while working so try to keep things unblocked     
  that isn't contingent on answers but i'll try to address things when i can. i 
  will typically ask for use of an interactive artifact (skill) when it's       
  complicated or lots of questions, but not always.                             
                                                                                
  milestone stops - yes for the agentic portion, but don't hard-wait for me at  
  every batch. do delivery the screenshots etc (use a scode-snippet), but don't 
  stop all work waiting for me unless something is SERIOUSLY off the rails.     
                                                                                
  i'm fine starting the executors myself with /goal                             
                                                                                
  regarding websocket/http and infallibility i'll address that in a moment.     

  Read 1 file, wrote 2 memories (ctrl+o to expand)

● I've saved all of this as settled decisions in my memory notes, so future
  sessions on the migration start from them, the no-parallel-app rule included.

  - One UI per helm. It gets enforced, and deferred if enforcing it gets
    complicated. Either way the code assumes a single UI and adds nothing to
    cope with several at once.
  - Plans in flight. Noted. I'll base the work on whatever main is when we start
    and check what the tmux-removal plans landed before the foundations goal
    begins.
  - Claude only. All agents run in Claude Code, so session-to-session pings work
    everywhere and no Codex polling path is needed.
  - ntfy. Notifications go to [notification topic] at normal priority. I'll send one
    when a batch of questions is waiting or a milestone's screenshots are ready,
    and otherwise keep everything moving that doesn't depend on an answer.
  - Milestones. At each milestone the agents pause for the complexity review
    only. I deliver the screenshots and the review's findings as a scode-snippet
    and resume without waiting for you, unless something is seriously off the
    rails.
  - Starting executors. You start each one with /goal.

  The last research report, on GPUI pitfalls, is still running. I'll hold the
  foundations list until you've weighed in on the HTTP/WebSocket question, since
  your answer affects how much of the request-handling foundation is needed.

✻ Worked for 28s · done 6:53 PM

❯ regarding websocket/http. So. This is one of the areas where agents added a   
  bunch of ridiculous complexity in the past, and there are 2 and 3 to actually 
  address this that we haven't gotten to yet.  Obviously, a remote Helm is not  
  actually infallible, but you can make an infallible-like system on top of     
  it. When the UI loses connection to the Helm, we can stipulate that when it   
  reconnects, it's a reset of the whole UI. We don't go and add a bunch of      
  insane complexity with individual actions, worrying about "Oh, the message    
  might get lost, the message might be out of order, blah blah blah blah blah,  
  all of that crap." We need to have a simple abstraction on top of which we    
  operate for the UI.                                                           
  I'm okay if we need some complexity to separate, for example, bulk data       
  transfer from low-latency, time-critical stuff like file transfers versus     
  clicks. But we cannot have every single individual action and click and       
  whatever having to deal with distributed systems problems around race         
  conditions, out-of-order delivery, and stuff like that.                       
                                                                                
  We need an abstraction that works as if there's an infallible, ordered stream 
  between the UI and the Helm. If that is actually failing, that needs some     
  kind of UI reset or something along those lines, so that we don't spread      
  complexity all over the place.                                                
                                                                                
  Please, let's talk about this a little bit more to make sure we can arrive at 
  a good place here.                                                            

❯ "2 and 3" -> there is a todo (voice dictation issue)                          

  Searched for 1 pattern, ran 1 shell command (ctrl+o to expand)

● I agree, and there's a clean way to make it true rather than a convention:
  make the connection the unit of failure. A single WebSocket runs over TCP, and
  TCP already delivers in order with no loss for as long as the connection is
  up. Messages go missing or arrive out of order in three situations:

  1. Requests travel on separate HTTP connections, so the helm can handle them
     in any order.
  2. Reads race the change feed. Most "an older list read brought back a deleted
     session" bugs come from this.
  3. The connection drops and the GUI tries to recover each action on its own.

  Remove all three and the infallible ordered stream isn't an approximation,
  it's what you actually have.

  (The TODO entry I found from 2026-10-26 says the same thing in your words:
  crash or start over, no per-action recovery.)

  The model I'd propose

  One UI channel. The GUI keeps a single WebSocket to the helm for everything
  except bulk data. It carries three things, all in one ordered stream:
  - the GUI's commands, each with an id;
  - the helm's reply to each command: done, refused, or accepted for long work;
  - the state the helm pushes: a full snapshot when the channel opens, then a
    fresh snapshot of each kind of data (sessions, hosts, templates, settings
    and so on) whenever it changes.

  The GUI never polls or re-reads anything. Its store is just the last thing the
  helm pushed. That makes the "older read" bug class impossible, because there
  are no reads.

  The helm keeps two promises:
  - It answers every command promptly. Work that takes long, such as host setup
    or trash deletion, replies "accepted" at once and reports progress as
    ordinary state. A timeout against a host is an ordinary reply ("the host
    timed out"), not a transport failure. The helm's links to supervisors stay
    fallible; that's real and the user sees it, but it never leaks into the
    transport.
  - Changes come before the reply. Any change a command causes is pushed before
    that command's reply. When the GUI sees "done", its state already shows the
    result, so no optimistic updates are needed. Locally the round trip is
    around a millisecond. For a remote helm, a control can show "working…" until
    the reply arrives.

  One failure path, for the whole UI:
  - A heartbeat checks the channel, and a single watchdog treats any command
    left unanswered for N seconds as a broken channel. That replaces all of
    today's per-request deadlines (the 10 s reads, the 60 s host operations, the
    15 min trash deletes).
  - When the channel breaks, the window goes inert behind a "Reconnecting to the
    helm…" screen. When it reconnects, everything that came from the helm is
    thrown away and rebuilt from the new snapshot. Terminals re-attach and
    replay from the supervisor as they do today.
  - One generic notice covers the gap: "The connection to the helm was lost;
    something you did just before may or may not have happened." This replaces
    SPEC.md's per-action "completes, or reports its outcome is unknown" rule. No
    action tracks its own fate.
  - It's the same code whether the helm is embedded or remote, and whether the
    helm crashed, restarted for an update, or the laptop slept.

  Bulk data gets its own connections, on purpose:
  - Terminal streams stay one stream per terminal, as today. They need their own
    reconnect handling anyway, because the supervisor and host behind them can
    fail independently.
  - File transfers (uploads and downloads) go over plain HTTP. A failed transfer
    is that transfer's own visible outcome, and the user retries it.
  - Neither touches the command path, so a large upload never delays a click.

  It also enforces one UI per helm. The UI channel is the seat: the helm accepts
  one, and a new UI displaces the old one. A displaced UI shows "another window
  took over" and doesn't reconnect on its own, like the terminal takeover rule.
  That's most of the enforcement you asked for, at little extra cost.

  Testing gets simpler. In tests the helm is a fake, ordered, in-process
  channel: deterministic, with no race to simulate except a dropped connection,
  and that's one test of the reset path.

  What it costs

  - On the helm side, this is new work. It needs the UI channel, the pushed
    snapshots, the promise that changes come before replies, and quick
    "accepted" replies for long operations. The HTTP API stays as it is for the
    command line and agents.
  - Simplest start: push the whole snapshot of a data kind every time it
    changes, not individual changes. At a few hundred sessions, that's small
    enough even for a remote helm. Sending only what changed goes on the watch
    list, triggered if bandwidth becomes a real problem.
  - On the GUI side, a whole layer disappears: lost-reply reconciliation, retry
    keys, the request fences that make a late reply lose, the outcome-unknown
    reports, and their tests. That's most of bug-history class 7 and a large
    share of class 2.

  Three things for you to decide

  1. What survives a reset. I'd keep local preferences (sidebar width, theme,
     window size) and the selected session, by id, if it still exists.
     Everything else starts over, including half-typed launcher drafts. Keeping
     drafts goes on the watch list, triggered if remote helms make drops common
     enough to hurt.
  2. Duplicates around a reset. A launch sent just before a drop may or may not
     have happened. After the reset the user sees the real list, with the new
     session in it if the launch went through. I'd accept the rare duplicate if
     they launch again without looking, and not bring back request keys to
     prevent it. Today's launch request keys and their retry rules go away.
  3. Whether the helm-side channel is in scope now. The alternative is to keep
     the current HTTP API plus change feed and push all the ordering into one
     client module in the GUI. That's less helm work, but it keeps a small
     version of the distributed-systems problem alive. I'd do the channel; the
     helm work is bounded and it removes the problem for good.

✻ Cooked for 1m 9s · done 6:56 PM

● Agent "Research GPUI pitfalls" finished · 22m 6s

● All three research reports are in. Below is my combined list: what to build
  first, what to prove early, and what goes on the watch list. The UI-channel
  item still depends on your answer to my last message.

  Two corrections from the research

  - Helm in-process: rejected. The bug-history report recommended calling the
    helm in-process. That contradicts keeping the helm/UI split.
  - "GPUI has no accessibility tree": wrong. GPUI has one (AccessKit), and the
    prototype used it. But a current GPUI bug drops the accessibility nodes of
    cached views, so caching and screen readers pull against each other. That
    matters only if screen readers matter to you.

  What the bug history showed

  386 GUI fix commits since August fall into about 19 classes. Focus is only one
  of them. The largest classes:

  1. An action runs on state the user wasn't shown: 47 to 55 fixes, still
     recurring on 2026-11-11. Cancel then a queued Confirm still installed; a
     row menu deleted the session next to it after rows moved.
  2. A late reply overrides newer state: an older list read brought back a
     deleted session.
  3. Popovers drifting from the row they belong to: mostly goes away in GPUI,
     because overlays anchor to elements every frame.
  4. Content unreachable at small window sizes.
  5. Host-supplied text with hidden characters.
  6. A terminal link opening something other than its hover showed.
  7. Lost replies and unknown outcomes: the UI channel removes this class.

  Also: drafts clobbered by refreshes, and operations owned by views. One class
  gets worse in GPUI: two copies of the same state that drift apart, because
  GPUI has no automatic change tracking.

  TODO.md's "Turning rules into structure" assessment (2026-10-19) reached the
  same conclusion: most fixes were "a rule one path followed and a sibling path
  missed".

  Foundations to build first

  1. UI channel and store (pending your answer): the ordered channel with pushed
     state, and one generic reset. The store copies what the helm pushes; views
     read from it when they draw.
  2. Update discipline.
     - Shared state changes only through methods that notify.
     - Caching goes only through one wrapper that compares the view's computed
       inputs.
     - A child view talks to its parent only by emitting events. A child calling
       back into a parent mid-update panics, and on macOS that panic aborts the
       whole app. All three analog apps the GPUI research found hit this.
     - Tests count redraws per view.
  3. Focus coordinator. Your rulebook as one module: the "moved on" counter,
     "home", and the only code that moves focus. In GPUI, moving focus also
     redraws the whole window and drops pending keystrokes, which is one more
     reason to keep it in one place.
  4. Layer manager. Dialog slot, popover stack and tooltips, all anchored to
     elements. Escape closes only the topmost. An overlay closes when its
     subject is gone, not when it moves. The covered-link check is a hit test,
     not a hand-kept list.
  5. One way to ask a question. A confirmation freezes what it shows when it
     opens, and its answer buttons come pre-wired to that frozen state. The helm
     checks the shown precondition when it acts. This is the structural fix for
     the biggest bug class.
  6. Operations owned by app-level stores, not views.
     - Tasks are kept in fields, which gives "latest wins" for free.
     - Feature code may not detach tasks.
     - UI crates may not spawn threads or other runtimes' tasks, sleep, or read
       the system clock, so tests stay deterministic.
  7. Drafts. Each draft field records whether the user touched it. A refresh
     never changes a touched field, and a save sends only touched fields. Covers
     the drafts class and the "a save undid an agent's change" class.
  8. Host-text type. Host-supplied text is its own type from the moment it's
     decoded. The only thing that can draw it is an element that escapes hidden
     characters and isolates text direction.
  9. Component layer over GPUI Kit, plus theme tokens.
     - Constructors require a label, hover help and a role.
     - Raw colors and direct Kit use are banned outside it.
     - One dialog frame keeps the answer buttons pinned and scrolls the body.
     - The window has a minimum size.
  10. Actions and keymap. One crate declares all actions. Buttons, menus and
      keys dispatch the same action. A terminal-passthrough table lists the keys
      the terminal always receives, and no context that contains a terminal may
      hold a multi-key chord prefix (GPUI holds the first key back for a
      second).
  11. One shared clock and animation ticker.
      - It ticks at most 10 times a second and pauses while the window is
        inactive.
      - It reads reduced motion from the OS itself.
      - No row runs its own timer.
      - Zed's own sidebar pins a CPU core this way, so this one is proven.
  12. Notice center. The single place that shows the reset notice, failure
      notices, approvals waiting to be answered, and sound.
  13. Startup and quit order.
      - One bootstrap sequence installs all app-wide state before anything runs.
      - Nothing in code that platform callbacks reach may panic (no unwrap or
        expect).
      - State is saved continuously, because GPUI gives quit handlers only 200
        ms.
  14. Test stack, ready before features and used by agents as they work:
      - model tests;
      - in-process GPUI tests, with:
        - a fake helm channel;
        - the seeded scheduler to reorder async completions;
        - focus and overlay checks after every step;
        - the real text system wherever layout matters (the fake one gives every
          character the same width);
      - the bridge driving the real app;
      - headless goldens plus real-window screenshots;
      - the screenshot-batch script;
      - a sweep with accessibility turned on, because duplicate element ids
        panic only then.
  15. Enforcement.
      - clippy disallowed-methods for each owner module above.
      - Crate layering.
      - A cargo-deny license allowlist. Avoid AGPL Cap, its patches included;
        it's the GPUI app with the most tempting code.
      - A short cookbook of GPUI traps for every agent. That doc is not a list
        of how the code is organized.

  Prove early

  These are risks of the kind you described, not implementation mistakes:

  - Terminal under a flood on real GPUs, plus how much memory text glyphs use.
    GPUI never evicts glyphs from its texture cache, so zooming grows memory;
    another GPUI terminal app reports 286 MB after 18 hours.
  - The session list while many rows animate.
  - Idle CPU on macOS: idle windows wake at display rate, up to 120 Hz, unless
    patched.
  - macOS 26 windows freezing after one frame. It's reported, and the upstream
    check looks unfixed. If it's in our pin, everything else waits on it.
  - NVIDIA on Wayland: device loss.

  Watch list

  Each item waits for its trigger:

  - Send only what changed instead of whole snapshots. Trigger: remote-helm
    bandwidth.
  - Keep drafts across a reset. Trigger: remote drops becoming common.
  - Multi-window. Keep focus and overlays per window, which is free, and build
    nothing else.
  - Undo toasts.
  - A strings module for translation.
  - A diagnostics journal beyond ordinary tracing.
  - A state-machine framework. The convention instead: each flow has one enum
    and one function that handles its events.
  - Screen-reader polish beyond roles and labels, i.e. live announcements and
    the caching conflict. Roles and labels stay as foundation work regardless,
    because the test bridge needs them.

  That last item is the one open question: how much do screen readers matter in
  their own right?

  Next I can draft the foundations-and-testing goal with scode-build-goal,
  including its up-front questions. Or first write this up as the GUI
  architecture section for SPEC_impl.md, so you can edit the rules directly.

✻ Cooked for 24m 53s · done 7:00 PM

❯ overall sounds good. for the decisions:                                       
                                                                                
  1. agree                                                                      
  2. yes, accept this                                                           
  3. no i agree do the channel and fix it properly.                             
                                                                                
  For your ofundation list, looks good but one thing: 12. The wording you used  
  here almost sounds like you're planning on designing a new visual layout. My  
  general intent is to basically duplicate precisely what it looks like today.  
  Can you elaborate on what you meant here?                                     
                                                                                
  for the nvidia on wayland stuff - i cannot promise a real physical linux box  
  with any particular hardware, we're gonna have to do without.                 
                                                                                
  for the "send only what changed instead of a whole snapshots" - i'm skeptical 
  here. the way i imagine it, sending what changed should be quite tractable,   
  and if it isn't, i worry I'm missing somethign about the architecture. can    
  you elaborate on why this is worth not just doing to start with?              
                                                                                
  also remind me what are tosts? ("undo taosts")                                
                                                                                
  screen readers: low priority utnil I have some non-trivial amount of users.   
  it's not worth it with near zero users besides myself.                        
                                                                                
  (dont draft goals quite yet)                                                  

● Noted on the three decisions: the channel, accepting the rare duplicate around
  a reset, and keeping only preferences and the selected session. Answers to
  the rest:

  Item 12, the notice center: no visual redesign. It's a code owner, not a new
  surface. Today the GUI already has several places that announce things:
  - the bell list and the notices panel,
  - approval cards,
  - banners such as the version-mismatch and outage cards,
  - the chimes.

  Each feature currently decides on its own when to post to them, when to
  withdraw a notice and when to play a sound. The notice center is the one
  module that makes those decisions, and the surfaces look exactly as they do
  today. The only new thing is the generic "connection was lost" notice after a
  reset, and it uses an existing notice style.

  The same applies to the whole plan: port, not redesign.
  - The theme tokens come from today's stylesheet.
  - Layouts copy today's.
  - Screenshots get checked side by side with the current app.

  Pixel-identical isn't possible, because text renders differently in GPUI, but
  layout, spacing, colors and behavior should match. Any deviation an agent
  can't avoid gets recorded and surfaced to you, not decided quietly. I'll make
  that an explicit rule.

  NVIDIA and Wayland. Accepted. Linux gets tested on software rendering in the
  VM and on whatever real hardware turns up. Real-GPU problems become a
  documented known-risk list, not a gate.

  Sending only what changed: you're right, do it from the start. My reason was
  only "start smaller", and it doesn't hold up. The GUI has to work out what
  changed anyway, so it can redraw only the rows that changed. That work has to
  happen somewhere, and doing it in the helm saves bandwidth too. It stays
  tractable if it's generic and coarse:
  - Each kind of data is a collection keyed by id.
  - The helm compares the new state with what it last sent this UI and sends
    added, replaced and removed entries.
  - An entry is always replaced whole. No field-level patches.
  - The GUI applies that with one generic piece of code, not per kind.

  The real risk with sending changes is a bug that silently leaves the GUI's
  copy different from the helm's until the next reset. That risk is cheap to
  cover: in debug builds and tests, the GUI periodically compares its copy
  against a full snapshot and fails loudly on any mismatch. So it moves off the
  watch list and into the channel foundation.

  Toasts. A toast is a small message that appears briefly, usually in a corner,
  and goes away on its own. An undo toast lets you take back an action you just
  took, such as "Session deleted — Undo", in place of asking for confirmation
  first. That would be a product change, not a port, so I'm dropping it from the
  list entirely.

  Screen readers: low priority. Roles and labels stay in the component
  constructors, because the test bridge reads them to find and describe
  controls, and they cost almost nothing there. Everything else moves to the
  watch list, with "a non-trivial number of users" as the trigger:
  - live announcements,
  - the conflict between caching and accessibility nodes,
  - the test sweep with accessibility turned on.

  The bridge doesn't depend on the OS accessibility tree, so testing loses
  nothing.

  I won't draft goals yet.
```
