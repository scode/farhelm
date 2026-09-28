#!/bin/sh
# Vercel's "Ignored Build Step" for the docs site (wired in by website/vercel.json).
#
# Exit 0 skips the deployment, exit 1 builds it, and ANY other exit status fails the deployment outright. That last
# rule is the trap: a bare `git diff --quiet` exits 128 when it cannot resolve a commit, and a failed deployment is not
# a "successful deployment", so VERCEL_GIT_PREVIOUS_SHA never advances and every later push fails the same way. So
# every path out of this script is exactly 0 or 1, and anything unexpected builds.
#
# Vercel runs this from the project's Root Directory (website/), so `-- .` means "anything under website/". The site
# must not read files outside website/, or a change to them would be skipped here.

# VERCEL_GIT_PREVIOUS_SHA is the branch's last successful deployment; a branch's first push has none, so compare with
# the parent commit instead. Comparing with the last deployment rather than the parent catches several merges landing
# between two deployments.
prev=${VERCEL_GIT_PREVIOUS_SHA:-HEAD^}

# Vercel clones shallowly, so a previous deployment far enough back is missing locally (observed as "bad object"). Fetch
# just that commit: git diff compares two trees and needs no history between them. The repository is public, so this
# needs no credentials; if the fetch fails anyway, the diff below fails and the push builds.
if ! git cat-file -e "$prev^{commit}" 2>/dev/null; then
  git fetch --quiet --depth=1 origin "$prev" 2>/dev/null
fi

if git diff --quiet "$prev" HEAD -- .; then
  echo "vercel-ignore-build: nothing under website/ changed since $prev; skipping"
  exit 0
fi
echo "vercel-ignore-build: website/ changed since $prev (or the comparison failed); building"
exit 1
