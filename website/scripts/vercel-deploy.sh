#!/bin/sh
# Deploy the docs site to Vercel on demand, from one exact commit on GitHub.
#
# Usage:
#   website/scripts/vercel-deploy.sh main          production deployment of GitHub's current main
#   website/scripts/vercel-deploy.sh <pr-number>   preview deployment of that PR's current head
#   website/scripts/vercel-deploy.sh --dry-run ... print the request body instead of sending it
#
# website/vercel.json turns off every Git-triggered deployment (`git.deploymentEnabled: false`), so nothing reaches
# Vercel unless someone runs this. The push-driven setup it replaces cost money for nothing: Vercel bills a deployment
# that its Ignored Build Step skips for at least a minute of build time, and nearly every push to this repository
# leaves the site unchanged.
#
# The deployment is built by Vercel from GitHub (the REST API's `gitSource`), the same build a push used to start.
# Nothing is uploaded from the local checkout, so uncommitted changes and local commits that were never pushed do not
# matter. The commit is resolved here and pinned in the request, so the deployment is of the head that was current when
# the script ran, even if a push lands while Vercel is still cloning.
#
# Needs gh and jq, plus a logged-in Vercel CLI (`vercel login`) whose current scope owns the `farhelm` project:
# `vercel api` reuses that login. There is no token handling here, and the team is never named in the repository.

usage() {
  echo "usage: $0 [--dry-run] main|<pr-number>" >&2
  exit 2
}

dry_run=
if [ "$1" = --dry-run ]; then
  dry_run=1
  shift
fi
[ $# -eq 1 ] || usage

# gitSource names the repository by owner and name. Taking it from gh rather than hard-coding it keeps the script
# working from any clone or jj workspace whose remote gh resolves.
repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner) || exit 1

case "$1" in
  main)
    ref=main
    sha=$(gh api "repos/$repo/commits/main" --jq .sha) || exit 1
    target=production
    ;;
  '' | *[!0-9]*)
    usage
    ;;
  *)
    pr=$(gh pr view "$1" --repo "$repo" --json headRefName,headRefOid,isCrossRepository) || exit 1
    # A fork's branch does not exist in this repository, so a gitSource naming it here would build the wrong thing or
    # nothing. Refusing is also the safer default: a preview build runs the PR's code with the project's settings.
    if [ "$(printf '%s' "$pr" | jq -r .isCrossRepository)" = true ]; then
      echo "vercel-deploy: PR $1 comes from a fork; only branches in $repo can be deployed" >&2
      exit 1
    fi
    ref=$(printf '%s' "$pr" | jq -r .headRefName)
    sha=$(printf '%s' "$pr" | jq -r .headRefOid)
    # Omitting `target` is what makes a deployment a preview.
    target=
    ;;
esac

body=$(jq -n \
  --arg repo "$repo" --arg ref "$ref" --arg sha "$sha" --arg target "$target" \
  '{
    name: "farhelm",
    project: "farhelm",
    gitSource: {type: "github", org: ($repo | split("/")[0]), repo: ($repo | split("/")[1]), ref: $ref, sha: $sha}
  } + (if $target == "" then {} else {target: $target} end)') || exit 1

if [ -n "$dry_run" ]; then
  printf '%s\n' "$body"
  exit 0
fi

echo "vercel-deploy: deploying $repo $ref at $sha (${target:-preview})" >&2
resp=$(printf '%s' "$body" | vercel api /v13/deployments -X POST --input - --raw) || exit 1
url=$(printf '%s' "$resp" | jq -r '.url // empty')
if [ -z "$url" ]; then
  echo "vercel-deploy: Vercel returned no deployment URL:" >&2
  printf '%s\n' "$resp" >&2
  exit 1
fi
printf '%s\n' "$resp" | jq -r '"vercel-deploy: created https://\(.url)\nvercel-deploy: inspect at \(.inspectorUrl)"' >&2

# Waiting here means the caller learns whether the deployment went live rather than only that it was queued. The
# timeout is generous so that a slow build queue is not reported as a failure.
vercel inspect "$url" --wait --timeout 10m
