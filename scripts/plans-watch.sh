#!/usr/bin/env bash
#
# Wait, without involving a model, until plans/ on the remote main branch changes.
#
# This is the idle half of "drain the plans and keep monitoring" and of "monitor for complete plans"
# (plans/AGENTS.md). A monitoring agent used to wake every hour (a background sleep) and spend a full model round
# discovering that nothing had changed; each such wake-up re-reads the agent's whole conversation uncached. This script
# does the cheap part instead: it polls the git tree hash of plans/ on the remote branch and exits only when that hash
# differs from the baseline the agent last drained against, so the agent's background-task completion is the wake-up.
#
# Why plans/ alone is enough: everything that can give an idle executor new work arrives as a change under plans/ on
# main, because the plans queue script (scripts/plans-queue.py) records every state change as a commit there. A new
# plan adds a line to plans/queue/INDEX.md; a plan answered, sent back for a follow-up, or given back returns to
# [pending]; a dependency becomes satisfied when its line is removed after landing; a finished plan becomes
# [complete] for the monitor to land. Merging or closing plan PRs makes nothing eligible by itself.
#
# Not every change under plans/ is worth a wake-up, though. With several executors, claims are the most frequent
# commits there and never give an idle executor work, and the monitor that lands finished plans cares only about plans
# becoming complete. With --wake-check, a changed tree hash is only a hint: the watcher then asks the queue script
# whether the difference from the baseline commit is something the agent's role acts on (--wake-for, executor by
# default), and keeps waiting if not. The tree hash stays the cheap first filter, so the check runs when the tree
# changes, not on every poll.
#
# The script only reads, through the GitHub API; it never touches a local repository, so it works the same from a
# colocated checkout or a jj workspace and cannot disturb either while it runs. It sticks to bash 3.2 features, the
# version stock macOS ships.
#
# Usage:
#   scripts/plans-watch.sh --repo OWNER/NAME (--baseline <tree-hash|none> | --baseline-from <commit-id>)
#                          [--branch main] [--interval SECONDS] [--max-wait SECONDS] [--max-failures N]
#                          [--request-timeout SECONDS] [--wake-check PATH [--wake-for executor|lander]]
#
# The baseline is the tree hash of plans/ in the main commit the agent's last round selected from, or `none` if that
# commit has no plans/ directory. --baseline-from <commit-id> asks GitHub for it, through the same request the polls
# use, which is the simplest way to get it right; --baseline takes a hash computed elsewhere. Either way it comes from
# the round's own commit rather than from the first poll: a plan that lands between the round's fetch and the
# watcher's start would otherwise become the baseline and never wake anyone.
#
# --wake-check PATH needs --baseline-from, since it compares against that commit. The watcher runs
# `PATH --repo OWNER/NAME wake-check --baseline <commit-id> --ref <branch> --for <role>` (scripts/plans-queue.py has
# exactly that interface), which must print `wake` or `ignore`. The role comes from --wake-for: `executor` for a
# draining executor, `lander` for the monitor. It runs under the same timeout and failure accounting as a request:
# a check that fails or prints anything else is a failed poll, never a change.
#
# Exit status and the one line printed to stdout (nothing goes to stderr, so a harness that captures only stdout, or
# merges both streams, still sees exactly one line):
#   0   `changed <baseline> <current>`: plans/ differs from the baseline (and, with --wake-check, the check said
#       `wake`); drain now.
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
wake_check=""
wake_for=executor
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
	--repo | --baseline | --baseline-from | --branch | --interval | --max-wait | --max-failures | --request-timeout | \
		--wake-check | --wake-for)
		[ $# -ge 2 ] || usage "$1 needs a value"
		case "$1" in
		--repo) repo=$2 ;;
		--wake-check) wake_check=$2 ;;
		--wake-for) wake_for=$2 ;;
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
case "$wake_for" in executor | lander) ;; *) usage "--wake-for must be executor or lander, got '$wake_for'" ;; esac
# Without a check there is nothing to pass the role to, and a monitor whose invocation lost --wake-check would wake on
# every change under plans/ without a word.
[ "$wake_for" = executor ] || [ -n "$wake_check" ] || usage "--wake-for needs --wake-check"
if [ -n "$wake_check" ]; then
	[ -n "$baseline_from" ] || usage "--wake-check needs --baseline-from: it compares against that commit"
	if ! [ -f "$wake_check" ] || ! [ -x "$wake_check" ]; then
		usage "--wake-check must name an executable file, got '$wake_check'"
	fi
fi

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

# Runs a command with stdout in $tmp/out and stderr in $tmp/err, cut off after --request-timeout. Returns its exit
# status, or leaves "request timed out" in $tmp/err and fails when the watchdog had to kill it.
#
# The timeout is a watchdog process rather than coreutils `timeout`, which stock macOS lacks. The watchdog traps TERM
# to take its own sleep down with it when the request finishes first.
bounded() {
	local req dog status
	"$@" >"$tmp/out" 2>"$tmp/err" &
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
	[ "$status" -lt 128 ] || echo "request timed out after ${request_timeout}s" >"$tmp/err"
	return "$status"
}

# Sets $result to the tree hash of plans/ at <ref> (a branch or commit id), or `none` when there is no plans/ directory
# there. Fails on any request error, on a hung request, on a truncated tree listing (plans/ could be the entry left
# out), or on a response that is neither shape, leaving a one-line reason in $tmp/err. Validating the shape is what
# keeps an error body or an empty response from being mistaken for "plans/ changed".
result=""
plans_tree() {
	local out
	bounded gh api "repos/$repo/git/trees/$1" \
		--jq 'if .truncated then error("tree listing truncated") else ([.tree[] | select(.path == "plans" and .type == "tree") | .sha][0] // "none") end' ||
		return 1
	out=$(cat "$tmp/out")
	if [ "$out" = none ] || is_object_id "$out"; then
		result=$out
		return 0
	fi
	echo "unexpected response: $(printf '%s' "$out" | head -c 100)" >"$tmp/err"
	return 1
}

# Sets $result to the --wake-check verdict, `wake` or `ignore`, for the branch against the baseline commit. Anything
# else, like a failed or hung check, is a failure with its reason in $tmp/err.
verdict() {
	local out
	bounded "$wake_check" --repo "$repo" wake-check --baseline "$baseline_from" --ref "$branch" --for "$wake_for" ||
		return 1
	out=$(cat "$tmp/out")
	case "$out" in
	wake | ignore)
		result=$out
		return 0
		;;
	esac
	echo "unexpected wake-check output: $(printf '%s' "$out" | head -c 100)" >"$tmp/err"
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

# One request (`plans_tree <ref>` or `poll`), with the failure accounting shared by the baseline lookup and the polls.
attempt() {
	local what=$1
	shift
	if "$@"; then
		failures=0
		succeeded=yes
		return 0
	fi
	failures=$((failures + 1))
	if [ "$failures" -ge "$max_failures" ]; then
		echo "error: $what failed $failures times in a row ($(last_error))"
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
	until attempt "reading the baseline at $baseline_from" plans_tree "$baseline_from"; do
		next_or_deadline
	done
	baseline=$result
	# Reading the baseline is not a poll of the branch: the deadline must not report `idle` before the branch has been
	# read at least once.
	succeeded=no
fi

# One poll: sets $current to the branch's plans/ tree hash and $decision to `wake` or `same`. With --wake-check,
# `ignored` is the last tree hash the check judged not worth waking for, so polls that keep seeing it do not re-run
# the check; any newer tree is checked against the baseline again, which also catches a claim that is later given
# back. The tree read and the check count as one attempt, so a check that keeps failing reaches --max-failures even
# though the tree reads between its failures succeed.
ignored=""
current=""
decision=same
poll() {
	decision=same
	plans_tree "$branch" || return 1
	current=$result
	if [ "$current" = "$baseline" ] || [ "$current" = "$ignored" ]; then
		return 0
	fi
	if [ -z "$wake_check" ]; then
		decision=wake
		return 0
	fi
	verdict || return 1
	if [ "$result" = wake ]; then
		decision=wake
	else
		ignored=$current
	fi
}

# The deadline poll is not skipped: a change that lands in the last interval still reports as `changed`.
while :; do
	if attempt "polling repos/$repo/git/trees/$branch" poll && [ "$decision" = wake ]; then
		echo "changed $baseline $current"
		exit 0
	fi
	next_or_deadline
done
