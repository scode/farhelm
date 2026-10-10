#!/usr/bin/env bash
#
# Drives scripts/plans-watch.sh as a child process against a stub `gh`, covering its whole exit contract.
#
# The watcher decides whether a monitoring plans agent wakes up at all (plans/AGENTS.md), so its failure modes are
# silent ones: a watcher that never reports a change leaves new plans unexecuted, one that reports changes that did
# not happen burns the model wake-ups it exists to save, and one that leaks processes or hangs outlives the agent that
# started it. Each case below pins one of those edges.
#
# The stub stands in for `gh api`, and it is deliberately strict so the tests exercise the real request rather than a
# canned answer: it accepts only commit and tree reads with their expected filters, logs the ref,
# and answers by running the watcher's own --jq filter through jq over a fixture tree listing shaped like GitHub's. A
# wrong endpoint, a broken filter, or a dropped truncation check therefore fails here. The stub is put first on the
# child's PATH; nothing changes this script's own environment.
#
# Each stub call consumes the next line of the case's response script; running out repeats the last line:
#   tree <hash>   a root listing with a plans/ directory at <hash> (plus unrelated entries)
#   tree none     a root listing without a plans/ directory (with a decoy file named `plans`)
#   truncated     a listing GitHub marked truncated
#   fail <text>   gh's failure: <text> on stderr, exit 1
#   raw <text>    <text> on stdout verbatim, exit 0 (a response gh's filter would never produce)
#   hang          a request that never answers
#
# Needs bash 3.2 or later, jq, and standard POSIX tools; runs on Linux and macOS. The stop cases need setsid, which
# stock macOS lacks, and are skipped there. About half a minute; no network.

set -u

here=$(cd "$(dirname "$0")" && pwd)
watch="$here/plans-watch.sh"
command -v jq >/dev/null 2>&1 || {
	echo "jq is required"
	exit 1
}
work=$(mktemp -d "${TMPDIR:-/tmp}/plans-watch-test.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT

# Hashes containing letters, so the uppercase cases really differ from the lowercase ones.
A=aaaaaaaaaa1111111111aaaaaaaaaa1111111111
B=bbbbbbbbbb2222222222bbbbbbbbbb2222222222
A_UPPER=AAAAAAAAAA1111111111AAAAAAAAAA1111111111
C=cccccccccc3333333333cccccccccc3333333333
P=eeeeeeeeee4444444444eeeeeeeeee4444444444
SHA256=dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd

failed=0
pass() { echo "ok   $1"; }
fail() {
	echo "FAIL $1: $2"
	failed=1
}

# --- Stub gh -----------------------------------------------------------------------------------------------------

make_stub() {
	local dir=$1
	mkdir -p "$dir/bin"
	: >"$dir/calls"
	: >"$dir/commit-calls"
	cat >"$dir/bin/gh" <<'STUB'
#!/usr/bin/env bash
dir=$(cd "$(dirname "$0")/.." && pwd)
if [ $# -ne 4 ] || [ "$1" != api ] || [ "$3" != --jq ]; then
	echo "BAD ARGV: $*" >>"$dir/calls"
	exit 98
fi
case "$2" in
repos/owner/name/git/ref/heads/*)
	[ "$4" = .object.sha ] || { echo "BAD COMMIT FILTER: $4" >>"$dir/calls"; exit 98; }
	echo "${2#repos/owner/name/git/ref/heads/}" >>"$dir/commit-calls"
	if [ -f "$dir/commit-error" ]; then cat "$dir/commit-error" >&2; exit 1; fi
	printf '{"object":{"sha":"eeeeeeeeee4444444444eeeeeeeeee4444444444"}}\n' | jq -r "$4"
	exit 0 ;;
repos/owner/name/git/trees/*) ref=${2#repos/owner/name/git/trees/} ;;
*) echo "BAD ENDPOINT: $2" >>"$dir/calls"; exit 98 ;;
esac
echo "$ref" >>"$dir/calls"
# In this case the branch advances after the immutable tree was read.
# The check must still inspect that tree's revision, rather than current main.
[ ! -f "$dir/race" ] || [ "$ref" != eeeeeeeeee4444444444eeeeeeeeee4444444444 ] || echo claimed >"$dir/branch-state"
n=$(wc -l <"$dir/calls")
total=$(wc -l <"$dir/responses")
[ "$n" -le "$total" ] || n=$total
line=$(sed -n "${n}p" "$dir/responses")
listing() {
	printf '{"sha":"%s","truncated":%s,"tree":[{"path":"README.md","type":"blob","sha":"%s"},%s]}\n' \
		"0000000000000000000000000000000000000000" "$1" "1234123412341234123412341234123412341234" "$2"
}
case "$line" in
"fail "*) echo "${line#fail }" >&2; exit 1 ;;
"raw "*) printf '%s\n' "${line#raw }"; exit 0 ;;
hang) exec sleep 30 ;;
"tree none") json=$(listing false '{"path":"plans","type":"blob","sha":"9999999999999999999999999999999999999999"}') ;;
"tree "*) json=$(listing false "{\"path\":\"plans\",\"type\":\"tree\",\"sha\":\"${line#tree }\"}") ;;
truncated) json=$(listing true '{"path":"plans","type":"tree","sha":"cccccccccc3333333333cccccccccc3333333333"}') ;;
*) echo "bad stub line: $line" >&2; exit 99 ;;
esac
printf '%s\n' "$json" | jq -r "$4"
STUB
	chmod +x "$dir/bin/gh"
}

case_dir() {
	local dir="$work/$1"
	make_stub "$dir"
	shift
	printf '%s\n' "$@" >"$dir/responses"
	echo "$dir"
}

# Runs the watcher in a case directory with the stub on PATH, recording stdout, stderr, and exit status. The working
# directory is the case directory, not a checkout, which also proves the watcher needs no local repository. Every
# run is bounded by a watchdog (status 124 if it fires), so a watcher that fails to exit fails its case instead of
# hanging the file.
run_case() {
	local dir=$1 pid dog status
	shift
	(cd "$dir" && exec env PATH="$dir/bin:$PATH" "$watch" --repo owner/name "$@" >"$dir/out" 2>"$dir/err") &
	pid=$!
	(
		trap 'kill "$s" 2>/dev/null; exit 0' TERM
		sleep 30 &
		s=$!
		wait "$s"
		kill "$pid" 2>/dev/null && echo fired >"$dir/watchdog"
	) &
	dog=$!
	wait "$pid"
	status=$?
	kill "$dog" 2>/dev/null
	wait "$dog" 2>/dev/null
	[ -f "$dir/watchdog" ] && status=124
	echo "$status" >"$dir/status"
}

calls() { tr '\n' ' ' <"$1/calls" | sed 's/ $//'; }
poll_count() { grep -c . "$1/calls"; }

# Checks status and stdout, that stderr is empty (the one-line contract), and that the stub saw only well-formed
# requests.
expect() {
	local name=$1 dir=$2 want_status=$3 want_out=$4 got_status got_out
	got_status=$(cat "$dir/status")
	got_out=$(cat "$dir/out")
	if [ "$got_status" != "$want_status" ] || [ "$got_out" != "$want_out" ]; then
		fail "$name" "status $got_status (want $want_status), stdout '$got_out' (want '$want_out')"
		return 1
	fi
	if [ -s "$dir/err" ]; then
		fail "$name" "unexpected stderr: $(cat "$dir/err")"
		return 1
	fi
	if grep -q '^BAD ' "$dir/calls"; then
		fail "$name" "malformed gh call: $(grep '^BAD ' "$dir/calls" | head -n 1)"
		return 1
	fi
	return 0
}

# Like expect, for exits whose stdout varies: status, a stdout pattern, and exactly one stdout line.
expect_line() {
	local name=$1 dir=$2 want_status=$3 pattern=$4
	if [ "$(cat "$dir/status")" = "$want_status" ] && [ "$(wc -l <"$dir/out")" -eq 1 ] &&
		grep -q -- "$pattern" "$dir/out" && ! [ -s "$dir/err" ] && ! grep -q '^BAD ' "$dir/calls"; then
		return 0
	fi
	fail "$name" "status $(cat "$dir/status") (want $want_status), stdout '$(cat "$dir/out")' (want /$pattern/), stderr '$(cat "$dir/err")', calls '$(calls "$dir")'"
	return 1
}

# --- Change detection ----------------------------------------------------------------------------------------------

# The core contract: unchanged polls keep waiting, and the first differing hash exits 0 naming both hashes. The stub
# runs the real filter, so this also proves the filter picks the plans/ tree entry out of a GitHub-shaped listing.
d=$(case_dir change "tree $A" "tree $A" "tree $B")
run_case "$d" --baseline "$A" --interval 1 --max-wait 60
if expect "change after unchanged polls" "$d" 0 "changed $A $B"; then
	if [ "$(calls "$d")" = "$P $P $P" ]; then pass "change after unchanged polls"; else
		fail "change after unchanged polls" "calls '$(calls "$d")'"
	fi
fi

# A change already present at start reports on the first poll: the baseline comes from the agent's last round, so a
# plan that landed before the watcher started is exactly what the baseline exists to catch.
d=$(case_dir immediate "tree $B")
run_case "$d" --baseline "$A" --interval 1 --max-wait 60
expect "change before start" "$d" 0 "changed $A $B" && pass "change before start"

# plans/ disappearing (with a decoy file of the same name, which the type filter must ignore) and appearing.
d=$(case_dir vanish "tree none")
run_case "$d" --baseline "$A" --interval 1 --max-wait 60
expect "plans/ removed, decoy file ignored" "$d" 0 "changed $A none" && pass "plans/ removed, decoy file ignored"
d=$(case_dir appear "tree none" "tree $A")
run_case "$d" --baseline none --interval 1 --max-wait 60
expect "plans/ created" "$d" 0 "changed none $A" && pass "plans/ created"

# --branch selects the ref polled.
d=$(case_dir branch "tree $B")
run_case "$d" --baseline "$A" --branch trunk --interval 1 --max-wait 60
if expect "custom branch" "$d" 0 "changed $A $B"; then
	if [ "$(cat "$d/commit-calls")" = trunk ] && [ "$(calls "$d")" = "$P" ]; then pass "custom branch"; else fail "custom branch" "calls '$(calls "$d")'"; fi
fi

# SHA-256 object ids are accepted as baselines and as responses.
d=$(case_dir sha256 "tree $SHA256" "tree $A")
run_case "$d" --baseline "$SHA256" --interval 1 --max-wait 60
expect "SHA-256 hashes" "$d" 0 "changed $SHA256 $A" && pass "SHA-256 hashes"

# --- Baseline from a commit ----------------------------------------------------------------------------------------

# --baseline-from reads the baseline at the given commit through the same request, then polls the branch.
d=$(case_dir from "tree $A" "tree $A" "tree $B")
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 60
if expect "baseline from commit" "$d" 0 "changed $A $B"; then
	if [ "$(calls "$d")" = "$C $P $P" ]; then pass "baseline from commit"; else
		fail "baseline from commit" "calls '$(calls "$d")'"
	fi
fi

# A commit without plans/ yields the `none` baseline.
d=$(case_dir from-none "tree none" "tree $A")
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 60
expect "baseline from commit without plans/" "$d" 0 "changed none $A" && pass "baseline from commit without plans/"

# Failing to read the baseline is retried like a poll and then reported, naming what failed.
d=$(case_dir from-fails "fail gh: Not Found (HTTP 404)")
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 60 --max-failures 2
expect_line "baseline read failure" "$d" 3 '^error: reading the baseline at .* (gh: Not Found (HTTP 404))$' &&
	pass "baseline read failure"

# A successful baseline read alone does not count as a poll: with every branch poll failing, the deadline reports an
# error, not `idle`.
d=$(case_dir from-then-fail "tree $A" "fail HTTP 502")
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 2 --max-failures 100
expect_line "baseline read is not a poll" "$d" 3 '^error: no successful request' && pass "baseline read is not a poll"

# --- Wake check --------------------------------------------------------------------------------------------------
#
# With --wake-check, a changed tree is only a hint and the check decides. The stub check logs its argv and answers
# from the case's verdict script the way the gh stub answers from its response script: `wake`, `ignore`, `fail
# <text>` (stderr, exit 1), `raw <text>`, or `hang`.

make_check() {
	local dir=$1
	shift
	printf '%s\n' "$@" >"$dir/verdicts"
	: >"$dir/checks"
	cat >"$dir/bin/wake-check" <<'STUB'
#!/usr/bin/env bash
dir=$(cd "$(dirname "$0")/.." && pwd)
echo "$*" >>"$dir/checks"
if [ -f "$dir/race" ]; then
	[ "$(cat "$dir/branch-state")" = claimed ] || exit 99
	case " $* " in
	*" --ref eeeeeeeeee4444444444eeeeeeeeee4444444444 "*) echo wake ;;
	*) echo ignore ;;
	esac
	exit 0
fi
n=$(wc -l <"$dir/checks")
total=$(wc -l <"$dir/verdicts")
[ "$n" -le "$total" ] || n=$total
line=$(sed -n "${n}p" "$dir/verdicts")
case "$line" in
"fail "*) echo "${line#fail }" >&2; exit 1 ;;
"raw "*) printf '%s\n' "${line#raw }" ;;
hang) exec sleep 30 ;;
*) echo "$line" ;;
esac
STUB
	chmod +x "$dir/bin/wake-check"
}
check_count() { grep -c . "$1/checks"; }

# An ignored tree is not re-checked on later polls that still see it; the next new tree is checked again and wakes.
# The check gets the repository, the baseline commit, and the immutable poll commit.
d=$(case_dir wake-ignore "tree $A" "tree $B" "tree $B" "tree $SHA256")
make_check "$d" ignore wake
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 60 --wake-check "$d/bin/wake-check"
if expect "wake check ignores, then wakes" "$d" 0 "changed $A $SHA256"; then
	if [ "$(check_count "$d")" -eq 2 ] &&
		[ "$(head -n 1 "$d/checks")" = "--repo owner/name wake-check --baseline $C --ref $P --for executor" ]; then
		pass "wake check ignores, then wakes"
	else
		fail "wake check ignores, then wakes" "checks: $(tr '\n' ';' <"$d/checks")"
	fi
fi

# The monitor's watcher asks the check about its own role: a check run as an executor's would wake it for work it
# never does and sleep through the plans it should land.
d=$(case_dir wake-lander "tree $A" "tree $B")
make_check "$d" wake
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 60 --wake-check "$d/bin/wake-check" --wake-for lander
if expect "wake check gets the lander role" "$d" 0 "changed $A $B"; then
	if [ "$(head -n 1 "$d/checks")" = "--repo owner/name wake-check --baseline $C --ref $P --for lander" ]; then
		pass "wake check gets the lander role"
	else
		fail "wake check gets the lander role" "checks: $(tr '\n' ';' <"$d/checks")"
	fi
fi

# Polls that see the baseline tree never run the check.
d=$(case_dir wake-unchanged "tree $A")
make_check "$d" wake
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 2 --wake-check "$d/bin/wake-check"
if expect "unchanged tree skips the check" "$d" 10 "idle $A"; then
	if [ "$(check_count "$d")" -eq 0 ]; then pass "unchanged tree skips the check"; else
		fail "unchanged tree skips the check" "ran $(check_count "$d") checks"
	fi
fi

# A claim between tree and verdict must not cache a newer verdict against
# the pending tree. The stub moves main before the check and only the frozen
# revision still offers work; asking main instead would idle here.
d=$(case_dir wake-revision "tree $A" "tree $B")
make_check "$d" ignore
touch "$d/race"
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 2 --wake-check "$d/bin/wake-check"
expect "wake check uses the tree's revision after main advances" "$d" 0 "changed $A $B" &&
	pass "wake check uses the tree's revision after main advances"

# Commit lookup failures share the bounded poll failure accounting; no tree
# read may turn an unresolved revision into an ignore or wake verdict.
d=$(case_dir commit-fails "tree $B")
echo 'HTTP 502 resolving main' >"$d/commit-error"
run_case "$d" --baseline "$A" --interval 1 --max-wait 60 --max-failures 2
expect_line "commit lookup failure" "$d" 3 'HTTP 502 resolving main' && pass "commit lookup failure"

# A failing check is a failed poll with its own message, and a garbled verdict is never a change.
d=$(case_dir wake-fails "tree $A" "tree $B")
make_check "$d" "fail error: gh is not on PATH"
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 60 --max-failures 2 --wake-check "$d/bin/wake-check"
expect_line "failing wake check" "$d" 3 '^error: polling .* failed 2 times in a row (error: gh is not on PATH)$' &&
	pass "failing wake check"
d=$(case_dir wake-garbled "tree $A" "tree $B")
make_check "$d" "raw maybe"
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 60 --max-failures 2 --wake-check "$d/bin/wake-check"
expect_line "garbled wake check" "$d" 3 'unexpected wake-check output: maybe' && pass "garbled wake check"

# A hung check is cut off like a hung request. The elapsed time is what shows the timeout worked: the stub's hang
# ends on its own after 30 s, so without a working timeout the case would still report the change, only much later.
d=$(case_dir wake-hang "tree $A" "tree $B")
make_check "$d" hang wake
started=$SECONDS
run_case "$d" --baseline-from "$C" --interval 1 --max-wait 60 --request-timeout 1 --wake-check "$d/bin/wake-check"
if expect "hung wake check times out" "$d" 0 "changed $A $B" && [ $((SECONDS - started)) -lt 10 ]; then
	pass "hung wake check times out"
else
	fail "hung wake check times out" "took $((SECONDS - started))s"
fi

# --- Idle cap ----------------------------------------------------------------------------------------------------

# With no change, the watcher exits 10 at --max-wait instead of running until a harness kills it, and it polls once
# more at the deadline rather than skipping the last interval.
d=$(case_dir idle "tree $A")
run_case "$d" --baseline "$A" --interval 1 --max-wait 2
if expect "idle at max-wait" "$d" 10 "idle $A"; then
	if [ "$(poll_count "$d")" -ge 2 ]; then pass "idle at max-wait"; else
		fail "idle at max-wait" "expected at least 2 polls, got $(poll_count "$d")"
	fi
fi

# A change seen at the deadline wins over idling. A zero-second budget puts the
# first poll at that boundary regardless of request overhead; requiring a third
# poll within two seconds instead measured machine speed, not this priority.
d=$(case_dir deadline "tree $B")
run_case "$d" --baseline "$A" --interval 1 --max-wait 0
expect "change on deadline poll" "$d" 0 "changed $A $B" && pass "change on deadline poll"

# --max-wait 0 is a single poll. Leading zeros are decimal, not octal (`08` would crash bash arithmetic).
d=$(case_dir once "tree $A")
run_case "$d" --baseline "$A" --interval 08 --max-wait 00
if expect "single poll, leading zeros" "$d" 10 "idle $A"; then
	if [ "$(poll_count "$d")" -eq 1 ]; then pass "single poll, leading zeros"; else
		fail "single poll, leading zeros" "got $(poll_count "$d") polls"
	fi
fi

# --- Failures ----------------------------------------------------------------------------------------------------

# Failures below the limit are retried silently; the change after them still reports.
d=$(case_dir transient "fail HTTP 502" "fail HTTP 502" "tree $B")
run_case "$d" --baseline "$A" --interval 1 --max-wait 60 --max-failures 3
expect "transient failures retried" "$d" 0 "changed $A $B" && pass "transient failures retried"

# The failure count is consecutive: a good poll in between resets it.
d=$(case_dir reset "fail HTTP 502" "fail HTTP 502" "tree $A" "fail HTTP 502" "fail HTTP 502" "tree $B")
run_case "$d" --baseline "$A" --interval 1 --max-wait 60 --max-failures 3
expect "failure count resets" "$d" 0 "changed $A $B" && pass "failure count resets"

# Persistent failure exits 3 with one error line that carries gh's own message, which is what tells an expired login
# from an outage.
d=$(case_dir persistent "fail gh: Bad credentials (HTTP 401)")
run_case "$d" --baseline "$A" --interval 1 --max-wait 60 --max-failures 3
expect_line "persistent failure names the cause" "$d" 3 \
	'^error: polling .* failed 3 times in a row (gh: Bad credentials (HTTP 401))$' &&
	pass "persistent failure names the cause"

# Failures that never reach --max-failures before --max-wait still end in an error, not `idle`: the agent must not
# keep restarting a watcher that has never once read the branch.
d=$(case_dir never-succeeded "fail HTTP 502")
run_case "$d" --baseline "$A" --interval 1 --max-wait 2 --max-failures 100
expect_line "no successful poll by the deadline" "$d" 3 '^error: no successful request within 2s (HTTP 502)$' &&
	pass "no successful poll by the deadline"

# Responses that are neither a hash nor `none` are failed polls, never changes: an error body, an uppercase or short
# hash, and an empty line.
d=$(case_dir malformed "raw <html>" "raw $A_UPPER" "raw 1234" "raw " "tree $A")
run_case "$d" --baseline "$A" --interval 1 --max-wait 5 --max-failures 5
expect "malformed responses are failures" "$d" 10 "idle $A" && pass "malformed responses are failures"
d=$(case_dir malformed-limit "raw <html>")
run_case "$d" --baseline "$A" --interval 1 --max-wait 60 --max-failures 2
expect_line "malformed responses count toward the limit" "$d" 3 'unexpected response: <html>' &&
	pass "malformed responses count toward the limit"

# A truncated listing is a failure, not `none`: plans/ could be the entry GitHub left out.
d=$(case_dir truncated truncated)
run_case "$d" --baseline "$A" --interval 1 --max-wait 60 --max-failures 2
expect_line "truncated listing is a failure" "$d" 3 'truncated' && pass "truncated listing is a failure"

# A hung request is cut off by --request-timeout and counts as a failure; the next poll still reports.
d=$(case_dir hang hang "tree $B")
started=$SECONDS
run_case "$d" --baseline "$A" --interval 1 --max-wait 60 --request-timeout 1
if expect "hung request times out" "$d" 0 "changed $A $B" && [ $((SECONDS - started)) -lt 10 ]; then
	pass "hung request times out"
else
	fail "hung request times out" "took $((SECONDS - started))s"
fi
d=$(case_dir hang-limit hang)
run_case "$d" --baseline "$A" --interval 1 --max-wait 60 --max-failures 1 --request-timeout 1
expect_line "hung request is reported as a timeout" "$d" 3 'request timed out after 1s' &&
	pass "hung request is reported as a timeout"

# --- Arguments -----------------------------------------------------------------------------------------------------

# Every bad invocation exits 2 with one `usage:` line and makes no request.
usage_case() {
	local name=$1 d
	shift
	d=$(case_dir "usage-$(echo "$name" | tr ' ' -)" "tree $A")
	(cd "$d" && env PATH="$d/bin:$PATH" "$watch" "$@" >"$d/out" 2>"$d/err")
	echo $? >"$d/status"
	expect_line "usage: $name" "$d" 2 '^usage: ' && [ "$(poll_count "$d")" -eq 0 ] && pass "usage: $name"
}
usage_case "missing repo" --baseline "$A"
usage_case "repo without owner" --repo name --baseline "$A"
usage_case "repo with extra segment" --repo a/b/c --baseline "$A"
usage_case "missing baseline" --repo owner/name
usage_case "both baselines" --repo owner/name --baseline "$A" --baseline-from "$C"
usage_case "short baseline" --repo owner/name --baseline 1234
usage_case "uppercase baseline" --repo owner/name --baseline "$A_UPPER"
usage_case "bad baseline-from" --repo owner/name --baseline-from main
usage_case "branch with slash" --repo owner/name --baseline "$A" --branch release/1
usage_case "branch with spaces" --repo owner/name --baseline "$A" --branch "ma in"
usage_case "zero interval" --repo owner/name --baseline "$A" --interval 0
usage_case "fractional interval" --repo owner/name --baseline "$A" --interval 0.5
usage_case "negative max-wait" --repo owner/name --baseline "$A" --max-wait -1
usage_case "huge max-wait" --repo owner/name --baseline "$A" --max-wait 99999999999
usage_case "zero max-failures" --repo owner/name --baseline "$A" --max-failures 0
usage_case "zero request timeout" --repo owner/name --baseline "$A" --request-timeout 0
usage_case "unknown argument" --repo owner/name --baseline "$A" --bogus
usage_case "missing value" --repo owner/name --baseline
usage_case "wake check without baseline-from" --repo owner/name --baseline "$A" --wake-check /bin/true
usage_case "wake check not executable" --repo owner/name --baseline-from "$C" --wake-check /nonexistent
usage_case "unknown wake role" --repo owner/name --baseline-from "$C" --wake-check /bin/true --wake-for maintainer
usage_case "wake role without a check" --repo owner/name --baseline-from "$C" --wake-for lander

# --- Stopping ----------------------------------------------------------------------------------------------------

# An agent stops its watcher when the maintainer stops the flow. It must exit promptly on SIGTERM whether it is sleeping between polls or waiting
# on a request, and leave nothing behind. The watcher runs in its own process group so the leftover check cannot match
# anything else on the machine, and the TERM goes to the watcher alone: signalling the group would kill the children
# directly and hide a watcher that orphans them.
stop_case() {
	local name=$1 response=$2 d launcher pgid status started
	d=$(case_dir "stop-$(echo "$name" | tr ' ' -)" "$response")
	(cd "$d" && exec env PATH="$d/bin:$PATH" setsid "$watch" --repo owner/name --baseline "$A" --interval 60 \
		--max-wait 600 --request-timeout 60 >"$d/out" 2>"$d/err") &
	launcher=$!
	for _ in $(seq 1 50); do
		[ "$(poll_count "$d")" -ge 1 ] && break
		sleep 0.1 # sleep-ok: polling interval while waiting for the stub's call log to show the first request
	done
	sleep 0.5 # sleep-ok: observation window so the watcher is inside its sleep or its request wait
	pgid=$(ps -o pgid= -p "$launcher" | tr -d ' ')
	started=$SECONDS
	kill -TERM "$launcher"
	wait "$launcher"
	status=$?
	sleep 0.3 # sleep-ok: observation window for leftover children to show before checking the group
	if [ $((SECONDS - started)) -le 2 ] && [ "$status" = 143 ] && ! [ -s "$d/out" ] && [ -n "$pgid" ] &&
		! pgrep -g "$pgid" >/dev/null 2>&1; then
		pass "stops promptly on SIGTERM $name"
	else
		fail "stops promptly on SIGTERM $name" "status $status after $((SECONDS - started))s, stdout '$(cat "$d/out")'; group $pgid: $(pgrep -g "$pgid" 2>/dev/null | tr '\n' ' ')"
		[ -n "$pgid" ] && pkill -g "$pgid" 2>/dev/null
	fi
}
if command -v setsid >/dev/null 2>&1; then
	stop_case "while sleeping" "tree $A"
	stop_case "during a request" hang
else
	echo "skip stops promptly on SIGTERM (no setsid command)"
fi

if [ "$failed" -ne 0 ]; then
	echo "FAILED"
	exit 1
fi
echo "all passed"
