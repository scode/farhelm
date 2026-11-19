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
