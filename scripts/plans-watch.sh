#!/usr/bin/env bash
#
# Wait, without involving a model, until plans/ on the remote main branch changes.
#
# This is the idle half of "drain the plans and keep monitoring" (plans/AGENTS.md). A monitoring agent used to wake
# every hour (a background sleep) and spend a full model round discovering that nothing had changed; each such wake-up
# re-reads the agent's whole conversation uncached. This script does the cheap part instead: it polls the git tree
# hash of plans/ on the remote branch and exits only when that hash differs from the baseline the agent last drained
# against, so the agent's background-task completion is the wake-up.
#
# Why plans/ alone is enough: everything that can give an idle executor new work arrives as a change under plans/ on
# main. A new plan adds an INDEX.md line; a dependency becoming satisfied flips its INDEX.md line to [executed]; a
# blocked plan the user revises changes its plan file. Answers given in the executor's own session and "check for
# plans" interrupt the agent directly. Merging or closing plan PRs makes nothing eligible: the next round reconciles
# them (step 1 of Executing) before it builds anything, so noticing them late costs nothing.
#
# The script only reads, through the GitHub API; it never touches a local repository, so it works the same from a
# colocated checkout or a jj workspace and cannot disturb either while it runs. It sticks to bash 3.2 features, the
# version stock macOS ships.
#
# Usage:
#   scripts/plans-watch.sh --repo OWNER/NAME (--baseline <tree-hash|none> | --baseline-from <commit-id>)
#                          [--branch main] [--interval SECONDS] [--max-wait SECONDS] [--max-failures N]
#                          [--request-timeout SECONDS]
#
# The baseline is the tree hash of plans/ in the main commit the agent's last round selected from, or `none` if that
# commit has no plans/ directory. --baseline-from <commit-id> asks GitHub for it, through the same request the polls
# use, which is the simplest way to get it right; --baseline takes a hash computed elsewhere. Either way it comes from
# the round's own commit rather than from the first poll: a plan that lands between the round's fetch and the
# watcher's start would otherwise become the baseline and never wake anyone.
#
# Exit status and the one line printed to stdout (nothing goes to stderr, so a harness that captures only stdout, or
# merges both streams, still sees exactly one line):
#   0   `changed <baseline> <current>`: plans/ differs from the baseline; drain now.
#   10  `idle <baseline>`: --max-wait elapsed with no change; restart with the same baseline. The cap exists because
#       agent harnesses bound how long a background command may run (Claude Code: two hours), and a watcher the
#       harness kills is indistinguishable from one that failed.
#   3   `error: <what> (<last gh error line>)`: --max-failures consecutive requests failed, or --max-wait elapsed
#       without a successful poll. Fewer consecutive failures are retried silently, since one dropped request is not
#       worth a model wake-up. The gh error line is what tells an expired login (HTTP 401) from an outage.
#   2   `usage: ...`: bad arguments.
#   143 (nothing printed): stopped by SIGTERM, SIGINT, or SIGHUP.

set -u

repo=""
baseline=""
baseline_from=""
branch="main"
interval=300
max_wait=6600
max_failures=3
request_timeout=60

usage() {
	echo "usage: $1"
	exit 2
}

# Stores a whole number, normalized to base 10, in the variable named $1. Bash arithmetic reads a leading zero as
# octal, so `08` would otherwise pass validation and then crash the first arithmetic expression that touches it.
set_uint() {
	case "$3" in '' | *[!0123456789]*) usage "$2 must be a whole number, got '$3'" ;; esac
	[ ${#3} -le 9 ] || usage "$2 is too large: '$3'"
	printf -v "$1" '%d' "$((10#$3))"
}

# A git object id as GitHub and `git rev-parse` print it: lowercase hex, 40 characters (SHA-1) or 64 (SHA-256). The
# characters are spelled out rather than written as a range, because a range like a-f can match uppercase letters
# under some locales' collation.
is_object_id() {
	[ ${#1} -eq 40 ] || [ ${#1} -eq 64 ] || return 1
	case "$1" in *[!0123456789abcdef]*) return 1 ;; *) return 0 ;; esac
}

# --- Arguments ---------------------------------------------------------------------------------------------------

while [ $# -gt 0 ]; do
	case "$1" in
	--repo | --baseline | --baseline-from | --branch | --interval | --max-wait | --max-failures | --request-timeout)
		[ $# -ge 2 ] || usage "$1 needs a value"
		case "$1" in
		--repo) repo=$2 ;;
		--baseline) baseline=$2 ;;
		--baseline-from) baseline_from=$2 ;;
		--branch) branch=$2 ;;
		--interval) set_uint interval "$1" "$2" ;;
		--max-wait) set_uint max_wait "$1" "$2" ;;
		--max-failures) set_uint max_failures "$1" "$2" ;;
		--request-timeout) set_uint request_timeout "$1" "$2" ;;
		esac
		shift 2
		;;
	*) usage "unknown argument '$1'" ;;
	esac
done

# The repository is required rather than discovered: discovery would be a network call at startup that the retry
# logic below does not cover, and it fails outright in a jj workspace, which has no .git for gh to read.
case "$repo" in
*/*/* | /* | */ | -* | *[![:alnum:]._/-]*) usage "--repo must be OWNER/NAME, got '$repo'" ;;
*/*) ;;
*) usage "--repo must be OWNER/NAME, got '$repo'" ;;
esac
if [ -n "$baseline" ] && [ -n "$baseline_from" ]; then
	usage "pass --baseline or --baseline-from, not both"
elif [ -n "$baseline" ]; then
	# A malformed baseline would compare unequal to every real hash and wake the agent on the first poll, which looks
	# like a change but is a caller bug.
	[ "$baseline" = none ] || is_object_id "$baseline" ||
		usage "--baseline must be a lowercase hex tree hash or 'none', got '$baseline'"
elif [ -n "$baseline_from" ]; then
	is_object_id "$baseline_from" || usage "--baseline-from must be a lowercase hex commit id, got '$baseline_from'"
else
	usage "pass --baseline or --baseline-from"
fi
# Plain names only. A slash would need URL-encoding in the request path, and gh sends it raw, so GitHub would answer
# a different route with 404 and the watcher would report polling errors instead of watching the branch.
case "$branch" in '' | -* | *[![:alnum:]._-]*) usage "--branch must be a branch name without '/', got '$branch'" ;; esac
[ "$interval" -ge 1 ] || usage "--interval must be at least 1"
[ "$max_failures" -ge 1 ] || usage "--max-failures must be at least 1"
[ "$request_timeout" -ge 1 ] || usage "--request-timeout must be at least 1"

# --- Children and signals ----------------------------------------------------------------------------------------
#
# Every child (a request, its timeout watchdog, the interval sleep) runs in the background and is waited on, with its
# pid recorded. That keeps two promises: a stop signal takes effect at once, because bash runs a trap as soon as a
# `wait` is interrupted but only after a foreground child exits; and the exit handler can kill exactly the children
# this script started, so stopping the watcher never leaves a request or a sleep behind. Nothing here runs inside a
# command substitution, which would be a foreground subshell and defer the trap again.

tmp=$(mktemp -d "${TMPDIR:-/tmp}/plans-watch.XXXXXX") || usage "could not create a temporary directory"
children=""
cleanup() {
	# shellcheck disable=SC2086 # deliberate word splitting: a space-separated pid list
	[ -n "$children" ] && kill $children 2>/dev/null
	rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 143' TERM INT HUP

nap() {
	sleep "$1" &
	children=$!
	wait "$children"
	children=""
}

# --- Requests ----------------------------------------------------------------------------------------------------

# Sets $result to the tree hash of plans/ at <ref> (a branch or commit id), or `none` when there is no plans/ directory
# there. Fails on any request error, on a hung request (after --request-timeout), on a truncated tree listing (plans/
# could be the entry left out), or on a response that is neither shape, leaving a one-line reason in $tmp/err.
# Validating the shape is what keeps an error body or an empty response from being mistaken for "plans/ changed".
#
# The timeout is a watchdog process rather than coreutils `timeout`, which stock macOS lacks. The watchdog traps TERM
# to take its own sleep down with it when the request finishes first.
result=""
plans_tree() {
	local req dog status out
	gh api "repos/$repo/git/trees/$1" \
		--jq 'if .truncated then error("tree listing truncated") else ([.tree[] | select(.path == "plans" and .type == "tree") | .sha][0] // "none") end' \
		>"$tmp/out" 2>"$tmp/err" &
	req=$!
	(
		trap 'kill "$s" 2>/dev/null; exit 0' TERM
		sleep "$request_timeout" &
		s=$!
		wait "$s"
		kill "$req" 2>/dev/null
	) &
	dog=$!
	children="$req $dog"
	wait "$req"
	status=$?
	kill "$dog" 2>/dev/null
	wait "$dog" 2>/dev/null
	children=""
	if [ "$status" -ne 0 ]; then
		[ "$status" -lt 128 ] || echo "request timed out after ${request_timeout}s" >"$tmp/err"
		return 1
	fi
	out=$(cat "$tmp/out")
	if [ "$out" = none ] || is_object_id "$out"; then
		result=$out
		return 0
	fi
	echo "unexpected response: $(printf '%s' "$out" | head -c 100)" >"$tmp/err"
	return 1
}

# The last non-empty line of the last failure, bounded to one short line so the stdout contract survives.
last_error() {
	grep -v '^[[:space:]]*$' "$tmp/err" 2>/dev/null | tail -n 1 | tr -d '\r\n' | head -c 200
}

# --- Main loop ---------------------------------------------------------------------------------------------------

failures=0
succeeded=no
start=$SECONDS

# One request, with the failure accounting shared by the baseline lookup and the polls.
attempt() {
	if plans_tree "$2"; then
		failures=0
		succeeded=yes
		return 0
	fi
	failures=$((failures + 1))
	if [ "$failures" -ge "$max_failures" ]; then
		echo "error: $1 failed $failures times in a row ($(last_error))"
		exit 3
	fi
	return 1
}

# Waits out the rest of the interval, or reports at the deadline. Shared by both loops so the deadline is one rule.
next_or_deadline() {
	local elapsed=$((SECONDS - start)) wait_for=$interval
	if [ "$elapsed" -ge "$max_wait" ]; then
		if [ "$succeeded" = no ]; then
			# Every request failed, but too few in a row to reach --max-failures (a --max-wait shorter than the
			# failure budget). Reporting `idle` would have the agent restart a watcher that has never seen GitHub.
			echo "error: no successful request within ${max_wait}s ($(last_error))"
			exit 3
		fi
		echo "idle $baseline"
		exit 10
	fi
	[ $((max_wait - elapsed)) -lt "$wait_for" ] && wait_for=$((max_wait - elapsed))
	nap "$wait_for"
}

if [ -n "$baseline_from" ]; then
	until attempt "reading the baseline at $baseline_from" "$baseline_from"; do
		next_or_deadline
	done
	baseline=$result
	# Reading the baseline is not a poll of the branch: the deadline must not report `idle` before the branch has been
	# read at least once.
	succeeded=no
fi

# The deadline poll is not skipped: a change that lands in the last interval still reports as `changed`.
while :; do
	if attempt "polling repos/$repo/git/trees/$branch" "$branch" && [ "$result" != "$baseline" ]; then
		echo "changed $baseline $result"
		exit 0
	fi
	next_or_deadline
done
