# Submenus open on hover

## What this was about

Using the pointer in Farhelm's desktop menus took a click before Mute or Move to group showed its choices. The parent
item also lost its highlight when the submenu opened. You chose to open submenus as soon as the pointer reaches them and
to keep the parent item highlighted while its submenu is open. The change applies to the session menu and the menu for
marked sessions.

## Things you should know

Hovering opens a submenu with focus on the parent item. Moving into the child, or pressing Right or Enter, moves focus
into the choices. Clicking the expanded parent leaves the submenu open and focus in place. Moving the pointer to another
parent item can switch submenus immediately. Left and Escape close the child, and hovering a menu leaves an open
confirmation in charge.

Enter and Space activate the highlighted command. If either arrives before a pointer-moved highlight reaches the screen,
it leaves the menu open. The user can press the key again after the highlight appears.

## Open questions and possible follow-ups

None.

## PRs

- Open desktop submenus on hover. [PR #4139](https://github.com/scode/farhelm/pull/4139/changes)

## Checks

- Focused `cargo nextest run -p farhelm-gui --lib -E <submenu component and exact row-menu regression>` passed all four
  tests in recorded run `dbd3200a-4278-4297-a092-b0520178ef17`.
- `cargo nextest run -p farhelm-gui --lib -E <component, session and header menus, focus, layers and prompts>` passed
  all 286 tests in recorded run `ac26a7d4-e66d-4bdb-8403-6fcf51f0badb`.
- `python3 scripts/check-gui-goldens.py` matched all 258 Linux screenshots in recorded run
  `a6bf2fe6-e99c-4a71-9ca1-56b2e204678a`.
- `python3 -B scripts/gui-e2e-smoke.py` passed 48 steps in recorded run `06da5585-4388-4abd-a73a-db3e8fb2d9d8`. The
  headless Linux compositor did not provide clipboard ownership, so the smoke skipped its clipboard step.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py` passed.
- Reused the application checks above from the same application diff based on `aece8416`. Main advanced to `13fc96db`
  through queue, review and release bookkeeping. After rebasing, `dprint check` on the changed Markdown,
  `python3 releasing/check-changelog.py format` and `git diff --check` passed again.
- Skipped the workspace-wide battery because the changes are confined to desktop menu and focus behavior. Native macOS
  checks need a Mac. The scoped tests simulated its Tab rules and the desktop smoke ran on Linux.

## Review gate

The fresh-context code review cleared after the fixes and regression tests. Its findings covered held Escape, Left on
the parent, Tab that leaves focus unchanged, and a press arriving before a pointer-moved highlight reaches the screen.
The commit message and PR wording passed the separate cold read.

### Landing

#### What landed with it

It landed first in a round of five plans. The others were clone-keeps-invisible-characters,
retire-old-terminal-announcement, quick-switcher-shot-paths and update-survival-wording. Since its base, main gained the
launcher's shared destination controls (#4130, #4133), docs screenshot and README hero capture changes, a docs refresh
and bookkeeping. None of it changes how menu rows are drawn or how the pointer highlights them. Two other plans in the
round also edit SPEC.md, in different sections, and the three edits do not contradict each other.

At your request this landing was reviewed with extra skepticism, both by me and by an independent reviewer, and I ran
the app to look at the submenus. In a session's menu they behave as the plan says:

- Hovering Mute opens its submenu at once, beside the item, with Mute still highlighted and nothing in the submenu
  highlighted.
- Moving the pointer into the submenu highlights the choice under it.
- Moving to Move to group switches to its submenu, and moving to Rename closes it.
- Right enters the submenu and Left returns.

The mechanism is fairly small for what it does. A hovered submenu opens without taking the keyboard, and one step hands
it the keyboard when the user enters it. It uses no timers.

#### Beyond the plan: a guard on Enter and Space in every menu

The executor's own review led it to add a guard the plan did not ask for. In every menu, not only submenus, Enter or
Space on a command is now ignored if it arrives after the pointer moved the highlight onto that command but before the
screen has shown it, a window of about one frame. The guard keeps a key from acting on an item the user has not yet seen
highlighted. It arguably follows SPEC.md's 2026-10-17 rule for input that arrives at nearly the same instant as another
move, but it closes a gap that existed before this plan (hovering already moved the highlight). It also widens a lint
exception: the GUI may ask to be told when the next frame is on screen, which until now only a reworded question did, to
keep an answer from authorizing words not yet shown. The guard is about 60 lines. Neither review found a bug in it.
Removing it is small, but it runs through the focus code, which I did not want to rework during a landing without the
executor, so I left it in. If you would rather not have it, removing it is a follow-up.

#### Fixes made while landing

- **Code comments that had lost their reasons.** The menu item's comment dropped why menus have no separate hover fill
  (a resting pointer would leave a second fill behind after an arrow-key move); it is back. A comment on menus and
  dialogs dropped that when one closes and the keyboard goes back where it was, a submenu that was only hovered, never
  entered, is skipped; it is back. A function described as running "when the pointer enters" an item now says "moves
  over", which is what it does since this plan.
- **SPEC.md's menu paragraphs.** The plan's rewrite dropped the sentence "A menu has one highlight. Whichever moved
  last, the pointer or the keys, sets it, and Enter acts on the highlighted item", which several code comments and tests
  quote. It also left the submenu paragraph unclear:
  - It said "hovering a submenu item", which reads as an item inside a submenu.
  - It did not say that Right or Enter opens a closed submenu.
  - It said "a dialog or question stays open on hover" without saying which hover.

  Both paragraphs now keep the old sentences. The new rules read plainly: submenus open on hover as a general rule for
  every menu, the parent stays highlighted, and hovering a menu never closes a dialog or question opened from it.
- **The GUI ownership document.** Its record of the lint exception the guard above uses had lost who granted it and when
  (the owner of the GUI's component layer and lint rules, on 2026-11-16), and it pointed at the wrong place in the code.
  Both are fixed, and the record now lists the Enter and Space guard as a second use.
- **A missing test.** Nothing checked that an arrow key moving off a hovered parent closes the submenu nobody entered.
  Without that, the old submenu's choices would stay beside the next item, and Right there would open a second submenu.
  The hover test now checks it.

#### Things you should know that the report does not say

- **Every menu item still shows a tooltip that repeats its description.** On hover it covers the row above. This
  predates the plan, but hover now matters more, because it also opens submenus. Possible follow-up: drop the tooltip
  where an item already shows its description.
- **No delay, as you chose.** The submenu opens level with its parent, so moving straight right from the parent into it
  stays on the parent the whole way. A diagonal move down toward a lower choice can cross the next item first, which
  closes the submenu; I did not try how often that happens in practice, nor try a Mac.

#### Checks

This round landed five plans together, so these checks cover all five on one tree: the five stacks one after another on
main. The plans change the desktop app's menus, launcher and quick switcher, the helm's release announcement table,
SPEC.md and documents. The landing ran:

- the lints and source checks;
- the whole test suites of the desktop app's GUI crate and of the helm;
- the app's end-to-end smoke;
- the reference screenshots (goldens);
- a hands-on run of the app, with screenshots of each submenu step (kept out of the repository).

Builds ran locally. The test suites and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered (main had gained only bookkeeping, the docs screenshot tooling, a docs refresh and the
launcher's shared destination controls since their bases):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui -p farhelm-helm`, from a locally built archive, split in three across boats: 1655,
  1603 and 1597 passed, 4855 in all (`1925a98b-8cce-47e4-ad2a-42f7d3d355ce`, `11bd4bb4-ae5f-45e9-a5a9-9be345c3a126`,
  `8a6fa767-830d-4071-a283-f25dea7661a1`). Three helm tests skipped at runtime because the archive does not carry a full
  `farhelm` binary: provisioning over SSH and over the local transport, and the uninstall stop. None touches these
  plans; that coverage is missing from this run.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 48 of its 49 steps passed and the clipboard
  paste step was skipped, as always on Linux (`9efe756f-4b33-4fe0-bd9a-02bffc4b3bdc`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`061c16ed-d3c3-4664-85d3-1d3340c34326`).

On the fixed tree (comments, SPEC.md, documents and two tests changed):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui --lib` for the session menu, submenu, Clone entry and quick switcher tests, locally:
  108 passed (`56e7255b-5924-40ff-bee9-03f8cf0bbe7f`), the two changed tests included.

The boat suites, the smoke and the goldens were reused for the fixed tree: after them only comments, SPEC.md, documents
and two tests changed, which the focused run above covers.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the plans stay in the GUI crate and the helm.
- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks: no Mac-specific code changed. The submenu behavior was checked on Linux only.
