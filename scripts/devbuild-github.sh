#!/usr/bin/env bash
# Check out a Farhelm pull request or branch from GitHub in
# /tmp/farhelm-devbuild and build it with scripts/devbuild-mac.sh, so the
# maintainer can try it on their Mac with one pasted line.
#
# Usage: curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/devbuild-github.sh | bash -s -- <PR number | branch> [--arch x86_64|aarch64]...
#
# An agent gives the maintainer that line with a PR's number, or with a
# branch name such as `main` (see "Trying a PR or branch on the maintainer's
# Mac" in AGENTS.md). An argument made only of digits is a PR number, so a
# branch named that way cannot be selected. Any `--arch` options pass
# through to devbuild-mac.sh. The build ends by printing "Run this to start
# farhelm:" and an absolute command that starts it from any directory;
# nothing is launched before the maintainer runs that.
#
# Commit hashes are deliberately not accepted. GitHub serves a fork's commits
# from this repository by hash once a PR has pulled them in, so a hash would
# bypass the fork refusal below, and a hash would not follow later pushes the
# way rerunning the same line for a PR or branch does.
#
# ## Idempotence
#
# Running it again, for the same target or another, is always safe and
# always ends in the same state: /tmp/farhelm-devbuild checked out at that
# PR's or branch's current head and built. The first run clones; later runs
# fetch the target and force the checkout to it, discarding any edits made
# in that directory, which belongs to this script. `target/` survives (it is
# gitignored, and `git clean` leaves ignored files alone), so later builds
# are incremental. The clone is made beside the destination and renamed into
# place, so an interrupted first run leaves no half-clone for the next run to
# trip over. Two runs at the same time are not supported.
#
# ## Trust
#
# Building runs the checked-out code, including its build scripts, on this
# Mac. A branch of scode/farhelm is the repository's own, but a PR can come
# from a fork, so the script refuses PRs whose branch lives outside
# scode/farhelm: a mistyped number must not run a stranger's code. It asks
# GitHub's API for that, unauthenticated, which is rate-limited to 60
# requests an hour per address; a refused lookup stops the script.
#
# NOTE: The script is written to run from a pipe. Everything is inside `main`,
# called on the last line, so a download cut short runs nothing, and `main`
# reads stdin from /dev/null, so no command it runs can swallow the rest of
# the script that bash is still reading from the pipe.

set -euo pipefail

slug=scode/farhelm
dir=/tmp/farhelm-devbuild

die() {
  echo "devbuild-github: $*" >&2
  exit 1
}

# Refuse a PR whose head branch is not in this repository; print its title.
#
# `plutil` ships with macOS and reads JSON, so this needs no jq or Python.
# A PR whose fork was deleted has a null head repository; the extraction then
# fails and the PR is refused, which is the safe answer.
check_pr_and_print_title() {
  local pr=$1 meta head_repo
  meta=$(curl -fsSL --proto '=https' -H 'Accept: application/vnd.github+json' \
    "https://api.github.com/repos/$slug/pulls/$pr") \
    || die "could not look up PR #$pr on GitHub (no such PR, no network, or rate-limited)"
  head_repo=$(printf '%s' "$meta" | plutil -extract head.repo.full_name raw -o - - 2>/dev/null) || head_repo=""
  [ "$head_repo" = "$slug" ] \
    || die "PR #$pr's branch is in '${head_repo:-a deleted repository}', not $slug; refusing to build code from outside the repository"
  printf '%s' "$meta" | plutil -extract title raw -o - - 2>/dev/null || true
}

main() {
  [ "$#" -ge 1 ] || die "usage: bash -s -- <PR number | branch> [--arch x86_64|aarch64]..."
  local what=$1
  shift
  [ "$(uname -s)" = Darwin ] || die "this builds the macOS desktop app; run it on a Mac"
  # Not `command -v git`: macOS ships a /usr/bin/git stub that exists without
  # the Command Line Tools and only fails, with xcrun's text, once run.
  xcode-select -p >/dev/null 2>&1 \
    || die "the Xcode Command Line Tools are missing; install them with: xcode-select --install"
  # Run once with its errors visible, so a git that cannot run at all (an
  # unaccepted Xcode license, say) says so here instead of surfacing below
  # as a rejected branch name.
  git --version >/dev/null || die "git does not run; see its error above"

  # `source` is the ref on GitHub; `label` names the target in messages.
  local source label title=""
  case "$what" in
    '' | -*) die "expected a PR number or a branch name, got '$what'" ;;
    *[!0-9]*)
      git check-ref-format --branch "$what" >/dev/null 2>&1 \
        || die "'$what' is neither a PR number nor a valid branch name"
      source=refs/heads/$what
      label="branch $what"
      ;;
    *)
      source=refs/pull/$what/head
      label="PR #$what"
      title=$(check_pr_and_print_title "$what")
      ;;
  esac

  # --- The checkout ----------------------------------------------------------
  #
  # A directory at the destination that is not a clone of this repository is
  # someone else's, and is left alone rather than force-checked-out.
  if [ ! -e "$dir" ]; then
    # Leftovers of interrupted first runs; see "Idempotence" above.
    rm -rf "$dir".clone.*
    echo "== cloning $slug into $dir"
    git clone --quiet "https://github.com/$slug.git" "$dir.clone.$$"
    mv "$dir.clone.$$" "$dir"
  fi
  # The `.git` test keeps `git -C` from answering for some enclosing
  # repository when the directory itself is not one. Symlinks are refused,
  # at the destination and at its `.git`, because the checkout and clean
  # below discard work: a link to a real clone of the repository would pass
  # every other test and lose that clone's edits and untracked files.
  # It reads the configured URL rather than `git remote get-url`, which
  # applies `url.<base>.insteadOf` rewrites and would report a rewritten URL
  # for the very clone made above.
  if [ -L "$dir" ] || [ -L "$dir/.git" ]; then
    die "$dir or its .git is a symlink; this script only resets a checkout it owns, so move it away and run this again"
  fi
  local origin=""
  if [ -d "$dir/.git" ]; then
    origin=$(git -C "$dir" config --get remote.origin.url 2>/dev/null) || origin=""
  fi
  case "$origin" in
    "https://github.com/$slug.git" | "https://github.com/$slug") ;;
    *) die "$dir exists but is not a clone of $slug (origin: '${origin:-none}'); move it away and run this again" ;;
  esac

  # Every target is fetched into the same private ref, by URL rather than
  # through the `origin` remote. Fetching through the remote would also update
  # its remote-tracking refs, which go stale (a deleted `foo` blocks a later
  # `foo/bar`) and can collide with each other, and a ref conflict would then
  # fail every rerun. One overwritten ref has nothing to conflict with.
  echo "== checking out $label${title:+: $title}"
  git -C "$dir" fetch --quiet --no-tags "https://github.com/$slug.git" "+$source:refs/devbuild/target" \
    || die "could not fetch $label from $slug"
  git -C "$dir" checkout --quiet --force --detach refs/devbuild/target
  git -C "$dir" clean --quiet -fd
  echo "   at $(git -C "$dir" log -1 --format='%h %s')"

  [ -x "$dir/scripts/devbuild-mac.sh" ] \
    || die "$label has no scripts/devbuild-mac.sh, so it predates the dev build; a PR or branch based on an older main needs a rebase"

  # devbuild-mac.sh prints the start command last; `exec` keeps that the last
  # thing on the terminal.
  exec "$dir/scripts/devbuild-mac.sh" --build-only "$@"
}

main "$@" </dev/null
