# Brainstorm: launching into a fresh GitHub clone

NOTE: Historical artifact, written 2026-09-17. Records the original launch-composer brainstorm and the
maintainer's follow-up decisions. This is historical discussion, not the current product specification.

The user and assistant text below is copied verbatim from the conversation, including spelling and formatting.
Role headings separate turns. Tool calls, tool results, and internal reasoning are omitted. The final turn records
the request to preserve the exchange and the additional configuration and ownership decisions.

## 1. User

i want to add a near term todo item. but let's brainstorm first.

I want to do something like the followingh but you can suggest better options if you want.

something that I'm doing a lot right now is that I want to start an agent, and I know that I want to run the agent in a separate working copy. I go and pick one of the existing agents, and I open a terminal window just to check out the repo. I can create the agent, and then I have to type the repo name, clone it, la da la, blah blah blah. I have to pick the repo that I want to clone. I want you to pick a local name for it because it's a different working copy of the same repo, so I need a different name. I need to name the session that's going to use it. 

At the same time, it's a hard requirement that farhelm remains repo-optional in the sense that it should allow you to just spawn agents, just like you do today, by default (without any built-in requirement that anything maps to a GitHub repo or other things like that) 

What I'm thinking is, let's add a concept of a modifier to the launch composer. you can imagine multiple kinds of modifiers, and in some ways, the existing way you can simply type the model name and hit Enter to select that model is a modifier. It's just not requiring you to do anything but name the model, and it'll automatically match. But in this case, I'm talking about something like a GitHub repo, which starts becoming very ambiguous, so if I just type foo/bar it's ambiguous whether I'm talking about a path that I want the agent to run in, perhaps, or maybe I'm talking about a repo, or maybe I'm talking about a provider and the model (if we add support in the future for "provider/model" for multi provider harnesses like opencode).

So I'm thinking *labled* modifiers. So if I want to launch an aggent but with an additional caveat "i want a disposeable working copy associated with this agent, and i want the github repo foo/bar checked out in it", I type "gh:foo/bar". This causes deterministic code to clone the repo into ~/git/bar-XXX where ~/git is a personal preference/config, and XXX is something auto-allocated but as readable as possible. So for example:

- if i did not pick a session name, a unique session name based on repo is created such as "bar-1", and XXX is "1"
- if i did pick a sesison name, then XXX is the session name, UNLESS the session name is prefixed by repo name in which case XXX is the suffix (excluding hyphen) of the session name (so e.g. if i name it "bar-fix" then XXX becomes fix if repo name was also bar).

What do you think?

## 2. Assistant

The direction is sound, but I'd change the framing and a few mechanics.

**Make `gh:` fill the folder slot rather than adding a general "modifier" concept.** The composer already has an optional folder choice. `gh:foo/bar` is a second way to fill it: "a fresh clone of X" instead of "an existing path". That keeps your repo-optional requirement at no cost, because the folder is already optional and the session model doesn't change.

I'd separate two things that "modifier" lumps together:

- **Labeled search scopes.** `gh:`, and later `dir:` or `model:`, disambiguate the search box. This part is cheap and general. Unlabeled input keeps today's fuzzy matching. `provider/model` can stay unlabeled because it resolves against the catalog, and `gh:` is what removes the ambiguity you describe.
- **Launches with side effects.** A model choice is pure selection, but a clone is slow, can fail, and needs credentials. It deserves its own design.

**The clone must happen on the target host.**

- Sessions can land on remote supervisors. The clone has to run there, with that host's git credentials and that host's base directory (`~/git` is per-host config).
- The helm should validate `owner/repo` with a strict pattern.
- The supervisor should construct the command. That matches the existing rule that the browser never produces argv fragments.

**I recommend running the clone inside the session's terminal before the agent starts**, roughly `git clone … && cd … && exec <agent>`.

- Progress, auth prompts and failures are then visible in the terminal as the clone runs.
- It avoids a new "preparing" session state.
- A failed clone leaves a session showing the error instead of a launch that hangs.
- A helm-side provisioning step with its own status would be much more work for the same result.

**Naming: your rule works, with these tweaks.**

- The rule reduces to: the directory is `slug(name)` if it starts with `bar-`, else `bar-` + `slug(name)`.
- With no session name, take the lowest free `bar-N` and name the session the same.
- Titles are renameable with no conflict detection, so the directory name is fixed at creation and may later differ from the title. That is fine, but the entry should say it's intentional.
- Allocate by atomic `mkdir` on the target host, not by checking first. Several agents and sessions race here.
- The session name needs a path-safe slug. If the explicit name collides with an existing directory, refuse and don't silently reuse it.
- Show the resolved path in the composer before launch, e.g. `→ ~/git/bar-fix on host`.

**"Disposable" is the risky word.** Auto-deleting a working copy that has unpushed work is how this feature loses someone's work. I suggest two steps:

- **v1:** never delete. Record that the session created the directory, as an ownership record like the uninstall work uses.
- **Later:** offer "remove working copy" on session close, guarded by a dirty/unpushed check.

**Smaller points**

- **Recent setups** should remember `gh:foo/bar` as the repo, so the next use makes a fresh clone. They should not remember the resulting folder.
- **Autocomplete for `gh:`** can come from recent repos plus the remotes of existing clones under the base directory. That needs no GitHub API or token.
- **Transport** (ssh or https, or `gh repo clone` when present) should be a per-host config value.
- **Post-clone hook:** you use jj, so an optional configured command (`jj git init --colocate`, installing hooks) is probably wanted early.
- **Clone speed and `gh:foo/bar@branch`** are natural follow-ups. For speed, `--reference-if-able` against a local mirror would work. Neither is needed for v1.
- **Scope:** I'd keep `gh:` built-in rather than building a generic "label → user recipe" system now. The hook covers most customization, and a generic recipe language is a much larger surface.

Two decisions shape the TODO entry:

- whether the clone runs in the terminal or as helm-side provisioning;
- whether v1 includes any deletion.

With those settled I can draft the near-term entry.

## 3. User

I definitely agree about the separation with respect to search scopes, right? Definitely agree with that part. Let's add that, but let's also make sure that it's not mandatory for the cases where we don't want it to be. For example, for model names, I don't want people to be required to prefix with `model:`, but prefixing should be fine.similarly, I don't want everything to be searchable without the prefix. Things like direct folders and repositories will often both match, and it's just going to get confusing. You want to keep the ones that are searchable without a label limited to the ones that are very unlikely to have overlap, where the behavior is quite deterministic. That way, you can work up the muscle memory, and it will, in practice, do the right thing (as opposed to randomly matching some other thing, like it matched the path instead of the repo, and you intended the repo). 

I agree about just running clone etc in the terminal. That is actually a really good user experience at very low cost, compared to what we would have to do to get the user experience to be good if we did something more fancy (which you're not proposing here).

I agree with everything else so far, with the clarification that when you say that `gh:` is built in, I interpret that to just mean it's hard-coded as a label in our Rust code (as opposed to being something the user can script), which I agree with. I don't want to completely hard-code it in the Rust code, though. I don't think that's what you mean. I'm just making it clear 

And I already, in terms of the decisions you want, mentioned I agree with the terminal containing the clone commands, and yes, I agree with V1 not including deletion for now, but I want to at least include some kind of move-to-trash operation. For safety, we will not actually delete, but we will move any session that would be deleted into a specific, reliably deterministic location in a directory. That way, it will behave as if we're deleting, except we're not actually deleting, and we're not freeing up disk space. Somebody can go and clean up. If you're fine with everything and you haven't felt like you lost something, you can safely go in and remove them. We get a signal on whether there is a bug right before we actually start adding real deletion. Let's use a fixed directory where all of those archived working copies end up, and let's call it "archived". By the way, I like that. Unless you have a better suggestion, let's just call it "archived working copies", basically.

The name, of course, will conflict because you might, over time, have multiple sessions with the same name and the same repo that get archived. I suggest that the naming of the subdirectory containing the working copy will simply be the original name, but suffixed with a date and the time (which represents when it was archived) and any additional random number in the unlikely event of a collision. 

The one thing I want to talk to you more about is the configuration aspect here, like the choice of ~/git and whether to use jj.

I have some ideas, but what is your suggestion for the simplest yet usable version of how this is configured

## 4. Assistant

My suggestion is two settings, both owned by the helm. Each has a global value with an optional per-host override, and I'd leave everything else unconfigurable in v1.

**1. `working_copy_root`** (for you, `~/git`)

- `~` is expanded on the target host. The per-host override covers a machine where it lives elsewhere.
- I would not ship a default, because this is a personal-layout choice and any default would be wrong for most people.
- The first time `gh:` is used against a host with no root set, the composer asks for it inline and remembers the answer. This is the same pattern as the remembered permissions mode.
- That way nobody has to find a settings page before first use, and people who never type `gh:` never see the concept.

**2. `post_clone`** (optional, one shell command string; for you, `jj git init --colocate`)

- It runs in the new directory, in the session's terminal, chained between the clone and the agent: `clone && cd && post_clone && exec agent`.
- Farhelm never has to know what jj is.
- A failed hook stops the launch, and the error shows in the session's terminal.
- The same slot handles hook installs, `direnv allow`, `npm ci` and similar.
- One global hook is coarse. If you later want it per repo, a script can branch on the directory it runs in, so the single string doesn't box us in.

**What I would leave out**

- **Transport.** I'd hardcode `https://github.com/{owner}/{repo}.git`. Git's `url.<base>.insteadOf` already lets a user rewrite that to ssh per host, and https works through a credential helper. We can add a URL template later if it turns out to be needed.
- **The archive location.** It is fixed at `<working_copy_root>/archived-working-copies/<name>-<YYYYMMDD-HHMMSS>[-N]`, per your "reliably deterministic" requirement. Putting it under the root also keeps it on the same filesystem, so archiving is a single atomic `rename`. If it crossed a mount it would become a slow copy-then-delete that can fail halfway.

Two consequences of that archive location:

- The `gh:` autocomplete scan of the root has to skip the archive directory.
- The allocator has to reserve its name, because repo `archived-working` plus session `copies` would produce exactly that path. A dot-prefixed name (`.archived-working-copies`) would avoid the collision, but it hides the directory from `ls`. I'd keep the visible name and reserve it.

**Why the helm owns the settings, not a file on each supervisor host**

- The helm already owns preferences, the host registry and the remembered launch state.
- It can send the resolved root and hook with the launch request, so the supervisor needs no new config reader.
- You also wouldn't have to ssh into each host to edit a file before `gh:` works there.

**Open question for you:** where do you edit `post_clone`? The root is covered by the inline first-use prompt, but a hook string needs a real editing surface. I didn't find a general settings surface in the spec, so I'm assuming there isn't one. The options I see are:

- a `farhelm` CLI subcommand, which is cheapest;
- a field in a per-host settings view;
- a small "working copy settings" disclosure inside the composer that appears once `gh:` is active.

I lean towards the CLI for v1. What were your own ideas? You said you had some.

## 5. User

This is almost exactly what I was thinking. I would suggest that instead of archived-working-copies we name it farhelm-archived-working-copies so its obvious when you just encounter it out of context.

I assume above that the "hook" you reference is thing we run in the termianl before the agent. So "post_clone" is a hook, "clone" is another? Or are you saying there is a single hook that runes multiple phases, one of which is post_clone?

For configuration, I want CLI - my general plan here is that we will add GUI to the things that are the most valuable to have in a GUI for being really fast, discoverable, and user-friendly. Over time, we'll build more and more functionality into the command-line tools in combination with skill instructions. For example, the user can just ask to configure this stuff, and the agent will figure out how to do it. We can have sort of wizard-like flows that can be used, and so on.

For these rarely configured things that don't need to be quick muscle memory, they're just persistent configuration. I don't want to spend a bunch of effort and complexity on building GUIs for it. The GUI is for the overall immediate, constant experience as you're using an agent, and for the quick muscle memory, discovery of the most muscle-memory-sensitive or just always immediately relevant things, and things that need to be really fast.

## 6. Assistant

Pure line-rewrap; letting the formatter do it.

## 7. Assistant

I've added the entry to the end of "Near term" in `TODO.md`. `dprint check` on `TODO.md` passes and nothing is committed.

**Your hook question:** there is one hook, `post_clone`. The clone is built-in behaviour and is not configurable. The terminal runs clone, then `cd`, then `post_clone` if one is set, then `exec`s the agent. The entry spells this out.

The directory name in the entry is `farhelm-archived-working-copies`, as you suggested.

**What I settled without asking**

- **Root not configured.** You want configuration CLI-only, so I dropped my inline first-use prompt. With no root set, `gh:` refuses to launch and names the CLI command that sets it.
- **Folders in unlabeled search.** Folders match unlabeled today. Your collision rule suggests moving them behind `dir:`, but that changes existing muscle memory. The entry leaves it as "part of this work" and does not claim a decision.
- **Archive trigger.** The entry says to offer "archive working copy" for sessions that created their working copy. It doesn't say where the action lives (session close, context menu, or CLI). That is an implementation detail, unless you want it settled now.
- **Out of scope.** The entry lists these as out of scope for v1: real deletion, `@branch`, reference-mirror clones, a clone URL template, and user-defined labels.

Tell me if you want any of these changed, or if you want it committed and opened as a PR.

## 8. User

btw please make a lore entry that contains our conversation above *verbatim* (exact original prompt by me and the resulting exchange). make the todo entry referne that lore entry as the original idea brainstorm.

for "root not configured", thats fine but please record a separate "Maybe later" todo item to re-consider adding the GUI flow on initial use or otherwise improve it.

for archive trigger: this should happen by default for any session that was spawned as an "owned" working copy. not sure we were explicit on that before but i consider there to be a difference here between a session that "owns" its working copy vs one that does not. that needs to be actually tracked in session metadata.

