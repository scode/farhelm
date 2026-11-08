#!/usr/bin/env python3
"""The landing monitor's round tooling: one command per mechanical step of a landing round.

NOTE: This does not decide anything. `plans/.agents/landing.md` owns the rules (which plans a round holds, when to
block, what the landing notes say, when to notify), and wins over anything here. These commands only carry out the
steps those rules leave mechanical, the same way every round, so that a monitor does not retype long jj and gh
sequences and cannot forget a guard. `scripts/plans-landing/README.md` lists the commands in the order a round uses
them.

The tooling used to be a set of ad hoc helpers in the monitor's scratch directory, with the checkout, the log, the
repository and the round number written into each file and rewritten by text substitution at every round start. A
reboot wiped them once already, and the substitution silently stopped updating the round base of the post-landing
comparison, which went on naming an earlier round's. Everything that varies is an argument now:

- `--checkout`: the monitor's checkout (default: the checkout this script lives in). Every jj and queue command runs
  there, one at a time; jj is never run concurrently.
- `--repo OWNER/NAME`: default from the checkout's `origin` remote.
- `--round-dir`: the round's own scratch directory, which `round-start` creates. It holds the round's inputs and
  outputs as plain text files so that a compacted or resumed session can read them back: `plans.txt` (one line per
  plan: slug, then its bookmarks bottom to top), `base.txt` (the main commit `queue status` printed at the round's
  start), `prs.txt` (slug, then `pr:bookmark` pairs bottom to top), `tested_commits.txt` (`commit bookmark` per
  bookmark, as the round's checks tested it), `diffs/` and `notes/`.

The monitor log is `farhelm-plans-monitor-log.md` in the checkout's parent directory, derived the way
`plans/.agents/outside-files.md` and `scripts/plans-queue.py` derive it. The commands that write it rely on two lines
the monitor keeps there: `running, last checked <time>` in `## Status`, and `- Landing claim in progress: <text>`.
History entries are appended at the end of the file.

Exit status: 0 done; 1 a step failed or refused (the message says which, and nothing after it ran); 2 usage error; 10
`claim` or `claim-land` found the plan no longer complete (`queue start-landing` exit 10), so the plan is skipped.

Run `scripts/plans-landing/test-landing.py` after changing this file.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime
import html
import json
import pathlib
import re
import secrets
import subprocess
import sys
import time

SCRIPT_CHECKOUT = pathlib.Path(__file__).resolve().parents[2]
MONITOR_LOG_NAME = "farhelm-plans-monitor-log.md"

EXIT_FAIL = 1
EXIT_USAGE = 2
EXIT_SKIP = 10

# The queue script's own state commits on main. They are bookkeeping, not landed work, so the cross-plan review and
# the post-landing comparison leave them out when listing what main gained.
NOT_BOOKKEEPING = "~ description(glob:'chore(plans):*')"

# Fetches keep commits whose remote branch is gone (a merged PR's head) instead of abandoning them silently, so that
# `round-end` can abandon exactly the landed heads it can name and nothing that belongs to someone else.
FETCH = ["git", "fetch", "--remote", "origin", "--config", "git.abandon-unreachable-commits=false"]

STATUS_RE = re.compile(r"^running, last checked .*$", re.M)
CLAIM_LINE_RE = re.compile(r"^- Landing claim in progress: .*$", re.M)


class Refusal(Exception):
    """A step failed or a guard refused; the message is printed and the command exits 1."""


# --- Paths, repository and monitor log ------------------------------------------------------------------------------


def monitor_log_path(checkout: pathlib.Path) -> pathlib.Path:
    """The monitor log beside the checkout, the same file `queue status` reads the landing claim's quiet time from."""
    return checkout.resolve().parent / MONITOR_LOG_NAME


def parse_github_repo(url: str) -> str | None:
    """`OWNER/NAME` from an https or ssh GitHub remote URL, or None when the URL is not a GitHub repository."""
    m = re.match(r"^(?:https://github\.com/|git@github\.com:|ssh://git@github\.com/)([\w.-]+/[\w.-]+?)(?:\.git)?/?$", url)
    return m.group(1) if m else None


def default_repo(checkout: pathlib.Path) -> str:
    """The `origin` remote's repository, read through git or, in a jj workspace without `.git`, through jj."""
    for cmd in (["git", "-C", str(checkout), "remote", "get-url", "origin"],
                ["jj", "-R", str(checkout), "git", "remote", "list"]):
        try:
            out = subprocess.run(cmd, capture_output=True, text=True, cwd=checkout).stdout
        except FileNotFoundError:
            continue
        for line in out.splitlines():
            parts = line.split()
            url = parts[-1] if parts and (len(parts) == 1 or parts[0] == "origin") else None
            if url and (repo := parse_github_repo(url)):
                return repo
    raise Refusal("cannot tell the repository from the origin remote; pass --repo OWNER/NAME")


def utc_minute() -> str:
    """The current UTC time to the minute, as every monitor log entry is stamped."""
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%MZ")


def with_status_refreshed(text: str, now: str) -> str:
    """The log with its `running, last checked` line set to `now`; refuses a log that has none (a stopped monitor)."""
    text, n = STATUS_RE.subn(f"running, last checked {now}", text, count=1)
    if n != 1:
        raise Refusal("the monitor log has no 'running, last checked' line in its Status section")
    return text


def with_history_entry(text: str, entry: str, now: str) -> str:
    """The log with one History entry appended; the entry is joined onto one line so the file never holds a stray
    continuation that dprint or a later edit would misread."""
    entry = " ".join(entry.split())
    if not entry:
        raise Refusal("empty log entry")
    if text and not text.endswith("\n"):
        text += "\n"
    return text + f"- {now}: {entry}\n"


def with_claim_line(text: str, claim: str) -> str:
    """The log with its `- Landing claim in progress:` line replaced; refuses a log that has none."""
    text, n = CLAIM_LINE_RE.subn(lambda _: f"- Landing claim in progress: {claim}", text, count=1)
    if n != 1:
        raise Refusal("the monitor log has no '- Landing claim in progress:' line")
    return text


def append_history(log: pathlib.Path, entry: str, refresh_status: bool = True) -> str:
    """Append a History entry stamped with the real current time, and by default refresh the Status line in the same
    write, so the two never disagree and no time is ever typed by hand. Returns the stamp."""
    now = utc_minute()
    text = log.read_text(encoding="utf-8")
    if refresh_status:
        text = with_status_refreshed(text, now)
    log.write_text(with_history_entry(text, entry, now), encoding="utf-8")
    return now


def set_claim_line(log: pathlib.Path, claim: str) -> None:
    """Rewrite the log's landing claim line, which a resume reads to settle a claim (landing.md, "Starting and
    resuming")."""
    log.write_text(with_claim_line(log.read_text(encoding="utf-8"), claim), encoding="utf-8")


# --- Round files ----------------------------------------------------------------------------------------------------


def read_plans(path: pathlib.Path) -> list[tuple[str, list[str]]]:
    """`plans.txt`: (slug, bookmarks bottom to top) per plan, in queue order. A plan with no bookmarks is refused,
    since every later step needs its bottom and top."""
    plans = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        slug, *bookmarks = line.split()
        if not bookmarks:
            raise Refusal(f"{path}: plan {slug} has no bookmarks")
        plans.append((slug, bookmarks))
    return plans


def read_pairs(path: pathlib.Path) -> dict[str, list[tuple[str, str]]]:
    """`prs.txt`: slug to its (PR number, bookmark) pairs, bottom to top."""
    pairs = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            slug, *items = line.split()
            pairs[slug] = [tuple(item.split(":", 1)) for item in items]
    return pairs


def tested_commit(path: pathlib.Path, bookmark: str) -> str:
    """The commit `tested_commits.txt` records for a bookmark. A trailing `*` (jj's mark for a bookmark that moved
    since its last push) is accepted, since that is how a list copied from `jj log` names it."""
    for line in path.read_text(encoding="utf-8").splitlines():
        commit, *names = line.split()
        if bookmark in (n.rstrip("*") for n in names):
            return commit
    raise Refusal(f"{path} records no tested commit for {bookmark}")


def claim_held(status: dict, slug: str, claim: str) -> bool:
    """Whether `queue status --json` shows this plan in `landing` under exactly this claim id. Checked before every
    push and merge: the maintainer may release a landing claim at any moment, and a released plan must not merge."""
    return any(p["slug"] == slug and p["state"] == "landing" and p["claim"] == claim for p in status["plans"])


# --- Running jj, gh and the queue -----------------------------------------------------------------------------------


@dataclasses.dataclass
class Ctx:
    """The checkout and repository every command works against. Every subprocess runs to completion before the next
    starts, which is what keeps jj commands from ever running concurrently in one repository."""

    checkout: pathlib.Path
    repo: str

    @property
    def log(self) -> pathlib.Path:
        return monitor_log_path(self.checkout)

    def run(self, cmd: list[str], check: bool = True, stdin: str | None = None) -> subprocess.CompletedProcess:
        r = subprocess.run(cmd, capture_output=True, text=True, cwd=self.checkout, input=stdin)
        if check and r.returncode != 0:
            raise Refusal(f"failed ({r.returncode}): {' '.join(cmd[:5])}\n{r.stdout[-800:]}{r.stderr[-800:]}")
        return r

    def jj(self, *args: str, check: bool = True) -> str:
        return self.run(["jj", "-R", str(self.checkout), *args], check=check).stdout

    def gh_json(self, *args: str):
        return json.loads(self.run(["gh", *args]).stdout)

    def queue(self, *args: str) -> subprocess.CompletedProcess:
        """`queue <verb>` from this checkout's own copy of the queue script, never raising: callers act on its exit."""
        return self.run([sys.executable, str(self.checkout / "scripts/plans-queue.py"), "--repo", self.repo, *args],
                        check=False)

    def status(self) -> dict:
        r = self.queue("status", "--json")
        if r.returncode != 0:
            raise Refusal(f"queue status failed ({r.returncode}): {r.stderr.strip()[-400:]}")
        return json.loads(r.stdout)

    def fetch(self) -> None:
        self.jj(*FETCH)


# --- Round start and the cross-plan review's inputs -----------------------------------------------------------------


def cmd_round_start(ctx: Ctx, args) -> None:
    """Fix the round's plans: every `[complete]` plan the round's `queue status` shows, in queue order, with its
    bookmarks tracked locally so the stack can be rebased. Refuses a dirty working copy: the clean-main boundary comes
    first, and its judgment about whose changes those are is not this tool's."""
    if ctx.jj("diff", "--summary", "-r", "@").strip():
        raise Refusal("the working copy has changes; apply the clean-main boundary first")
    rd = args.round_dir
    if rd.exists():
        raise Refusal(f"{rd} already exists; each round gets a new directory")
    ctx.fetch()
    status = ctx.status()
    lines = []
    for plan in status["plans"]:
        if plan["state"] != "complete":
            continue
        slug = plan["slug"]
        ctx.jj("bookmark", "track", f"plan/{slug}/*", "--remote", "origin", check=False)
        names = sorted(set(ctx.jj("bookmark", "list", "-a", f"plan/{slug}/*", "-T", 'name ++ "\\n"').split()))
        if not names:
            # A complete plan with nothing open is one of `queue status`'s flags, handled per landing.md, not stacked.
            print(f"{slug}: complete with no plan bookmarks; act on its status flag instead", file=sys.stderr)
            continue
        lines.append(f"{slug} {' '.join(names)}")
    # Created only now, so a failed fetch or status leaves nothing behind and the same command can simply be rerun.
    rd.mkdir(parents=True)
    (rd / "notes").mkdir()
    (rd / "diffs").mkdir()
    (rd / "base.txt").write_text(status["head"] + "\n", encoding="utf-8")
    (rd / "plans.txt").write_text("".join(f"{line}\n" for line in lines), encoding="utf-8")
    print(f"head {status['head']}")
    print("\n".join(lines) if lines else "no [complete] plans: the round is over")


def cmd_bases(ctx: Ctx, args) -> None:
    """Per plan: its pushed base, the non-bookkeeping main commits since then (landed work it has not seen), and any
    path under `plans/` its stack touches (landing.md step 3: no plan PR may touch `plans/`)."""
    for slug, bms in read_plans(args.round_dir / "plans.txt"):
        base = ctx.jj("log", "--no-graph", "-r", f"{bms[0]}@origin-", "-T", "commit_id.short(8)").strip()
        since = ctx.jj("log", "--no-graph", "-r", f"{base}..main@origin {NOT_BOOKKEEPING}",
                       "-T", 'description.first_line() ++ "\\n"').splitlines()
        touched = [line for bm in bms for line in ctx.jj("diff", "--summary", "-r", bm).splitlines()]
        plans_touch = [t for t in touched if t.split()[-1].startswith("plans/")]
        print(f"{slug} base {base}; main since: {len(since)}; plans/ touched: {plans_touch or 'none'}")
        for subject in since:
            print("   ", subject)


def cmd_stack(ctx: Ctx, args) -> None:
    """Stack the round's plans in queue order on main@origin (each plan's bottom onto the previous plan's top) and
    list every conflicted commit with its files, so bookkeeping-only conflicts can be told from real ones before
    anything is resolved."""
    plans = read_plans(args.round_dir / "plans.txt")
    if not plans:
        raise Refusal("plans.txt is empty")
    onto = "main@origin"
    for _slug, bms in plans:
        ctx.jj("rebase", "-s", bms[0], "-d", onto)
        onto = bms[-1]
    top = plans[-1][1][-1]
    out = ctx.jj("log", "--no-graph", "-r", f"main@origin::{top} & conflicts()", "-T", 'bookmarks ++ "\\n"')
    for bm in out.split():
        bm = bm.rstrip("*")
        files = ctx.jj("resolve", "--list", "-r", bm, check=False).splitlines()
        print(bm, "|", " ".join(f.split()[0] for f in files if f.strip()))
    print("TOP", top)


def cmd_diffs(ctx: Ctx, args) -> None:
    """Write the cross-plan review's inputs to `diffs/`: each plan's diff as stacked locally (so it includes the
    landing's resolutions and fixes), and main's landed work since the oldest base, without `plans/`."""
    rd = args.round_dir
    (rd / "diffs").mkdir(exist_ok=True)
    for slug, bms in read_plans(rd / "plans.txt"):
        text = ctx.jj("diff", "--git", "--from", f"{bms[0]}-", "--to", bms[-1])
        (rd / "diffs" / f"{slug}.diff").write_text(text, encoding="utf-8")
        print(slug, len(text.splitlines()))
    landed = ctx.jj("diff", "--git", "--from", args.oldest_base, "--to", "main@origin", "~plans/")
    (rd / "diffs" / "landed-since-bases.diff").write_text(landed, encoding="utf-8")
    print("landed-since-bases", len(landed.splitlines()))


def cmd_pr_map(ctx: Ctx, args) -> None:
    """Write `prs.txt`: each bookmark's open PR. Refuses a bookmark with no open PR or more than one, because a
    landing must name exactly the plan's own PRs."""
    lines = []
    for slug, bms in read_plans(args.round_dir / "plans.txt"):
        pairs = []
        for bm in bms:
            found = ctx.gh_json("pr", "list", "-R", ctx.repo, "--head", bm, "--state", "open", "--json", "number")
            numbers = [p["number"] for p in found]
            if len(numbers) != 1:
                raise Refusal(f"{bm}: expected one open PR, found {numbers}")
            pairs.append(f"{numbers[0]}:{bm}")
        lines.append(f"{slug} {' '.join(pairs)}")
    (args.round_dir / "prs.txt").write_text("".join(f"{line}\n" for line in lines), encoding="utf-8")
    print("\n".join(lines))


def cmd_tested(ctx: Ctx, args) -> None:
    """Write `tested_commits.txt`: every bookmark's current commit. Run it when the round's combined checks start, so
    `verify` can later compare main with the tree the checks actually saw."""
    lines = []
    for _slug, bms in read_plans(args.round_dir / "plans.txt"):
        for bm in bms:
            lines.append(f"{ctx.jj('log', '--no-graph', '-r', bm, '-T', 'commit_id').strip()} {bm}")
    (args.round_dir / "tested_commits.txt").write_text("".join(f"{line}\n" for line in lines), encoding="utf-8")
    print("\n".join(lines))


# --- Landing one plan -----------------------------------------------------------------------------------------------


def merge_one(ctx: Ctx, slug: str, claim: str, pr: str, bm: str, want: str) -> str:
    """Guarded squash merge of one plan PR (landing.md, "A landing round", step 5).

    Before every attempt it confirms the landing claim is still this monitor's and that the PR is open, based on main,
    and headed by exactly the commit that was pushed and tested; the merge passes `--match-head-commit`, so GitHub
    refuses if the branch moved underneath. Main moves often through bookkeeping commits, so a refusal is re-read and
    retried a few times rather than treated as fatal. Success is reported only once GitHub shows the PR as MERGED,
    since `gh pr merge` can exit 0 without merging.
    """
    for attempt in range(1, 7):
        if not claim_held(ctx.status(), slug, claim):
            raise Refusal(f"claim {claim} lost before merging #{pr}")
        view = ctx.run(["gh", "pr", "view", pr, "-R", ctx.repo, "--json", "state,baseRefName,headRefName,headRefOid"],
                       check=False)
        if view.returncode != 0:
            time.sleep(10)
            continue
        info = json.loads(view.stdout)
        if info["state"] == "MERGED":
            return f"MERGED {pr} head={want} (already)"
        seen = (info["state"], info["baseRefName"], info["headRefName"], info["headRefOid"])
        if seen != ("OPEN", "main", bm, want):
            print(f"unexpected PR state for #{pr}: {' '.join(seen)} (want OPEN main {bm} {want})", flush=True)
            time.sleep(10)
            continue
        merge = ctx.run(["gh", "pr", "merge", pr, "-R", ctx.repo, "--squash", "--match-head-commit", want],
                        check=False)
        if merge.returncode == 0:
            state = ""
            for _ in range(6):
                state = ctx.gh_json("pr", "view", pr, "-R", ctx.repo, "--json", "state")["state"]
                if state == "MERGED":
                    return f"MERGED {pr} head={want}"
                time.sleep(5)
            raise Refusal(f"merge of #{pr} reported success but its state is {state}")
        print(f"attempt {attempt} refused: {merge.stderr.strip()[:300]}", flush=True)
        time.sleep(15)
    raise Refusal(f"giving up on #{pr}")


def land_plan(ctx: Ctx, slug: str, claim: str, pairs: list[tuple[str, str]]) -> None:
    """Land one claimed plan's PRs bottom-up: rebase each onto main@origin, refuse on conflicts, retarget the PR to
    main, confirm the claim, push, wait until GitHub shows the pushed head, mark it ready and merge it through
    `merge_one`. Each merge is appended to the monitor log. Leaves the plan in `landing`; notes and `queue landed`
    are the caller's."""
    for pr, bm in pairs:
        ctx.fetch()
        print(ctx.run(["jj", "-R", str(ctx.checkout), "rebase", "-s", bm, "-d", "main@origin"]).stderr.strip()[-300:])
        marks = ctx.jj("log", "-r", f"main@origin::{bm}", "--no-graph", "-T", 'if(conflict, "CONFLICT\\n", "")')
        if "CONFLICT" in marks:
            raise Refusal(f"conflict at {bm}")
        retarget = ctx.run(["gh", "api", "-X", "PATCH", f"repos/{ctx.repo}/pulls/{pr}", "-f", "base=main"],
                           check=False)
        if retarget.returncode != 0:
            base = ctx.gh_json("pr", "view", pr, "-R", ctx.repo, "--json", "baseRefName")["baseRefName"]
            if base != "main":
                raise Refusal(f"retargeting #{pr} to main failed")
        if not claim_held(ctx.status(), slug, claim):
            raise Refusal(f"claim {claim} lost before pushing {bm}")
        # The push's own exit is not trusted either way: the head check below is what decides.
        push = ctx.run(["jj", "-R", str(ctx.checkout), "git", "push", "--bookmark", f"exact:{bm}"], check=False)
        print((push.stdout + push.stderr).strip()[-300:])
        want = ctx.jj("log", "-r", bm, "--no-graph", "-T", "commit_id").strip()
        head = ""
        for _ in range(20):
            head = ctx.gh_json("pr", "view", pr, "-R", ctx.repo, "--json", "headRefOid")["headRefOid"]
            if head == want:
                break
            time.sleep(3)
        if head != want:
            raise Refusal(f"GitHub does not show the pushed head {want} for #{pr} (shows {head})")
        ctx.run(["gh", "pr", "ready", pr, "-R", ctx.repo])
        # Mergeability is computed lazily after a push; wait for GitHub to decide before the first merge attempt.
        for _ in range(40):
            m = ctx.gh_json("pr", "view", pr, "-R", ctx.repo, "--json", "mergeable,mergeStateStatus")
            if "UNKNOWN" not in (m["mergeable"], m["mergeStateStatus"]):
                break
            time.sleep(5)
        print(merge_one(ctx, slug, claim, pr, bm, want), flush=True)
        # Not refreshing Status here: a log edit that failed after a merge must not stop the rest of the landing.
        append_history(ctx.log, f"{slug} merged #{pr}", refresh_status=False)
    print("ALL-MERGED")


def verify_landed(ctx: Ctx, rd: pathlib.Path, top: str) -> None:
    """Compare main with the tree the round's checks tested at a plan's top bookmark. Once a plan's PRs have all
    merged, main should equal that commit apart from `plans/` and whatever unrelated commits main gained since the
    round's base, which are listed by subject so each difference can be attributed."""
    tested = tested_commit(rd / "tested_commits.txt", top)
    base = (rd / "base.txt").read_text(encoding="utf-8").strip()
    diff = [line for line in ctx.jj("diff", "--summary", "--from", tested, "--to", "main@origin").splitlines()
            if line.strip() and not line.split()[-1].startswith("plans/")]
    print(f"main@origin vs tested {tested[:12]} ({top}), outside plans/: {len(diff)} file(s)")
    for line in diff:
        print("  ", line)
    others = ctx.jj("log", "--no-graph", "-r", f"{base}..main@origin {NOT_BOOKKEEPING}",
                    "-T", 'commit_id.short(8) ++ " " ++ description.first_line() ++ "\\n"')
    print("non-bookkeeping main commits since the round base:")
    print(others, end="")


def round_pairs(rd: pathlib.Path, slug: str) -> list[tuple[str, str]]:
    """A plan's PRs from `prs.txt`; a plan that is not there is refused before anything is claimed."""
    pairs = read_pairs(rd / "prs.txt").get(slug)
    if not pairs:
        raise Refusal(f"{rd / 'prs.txt'} has no PRs for {slug}")
    return pairs


def claim_plan(ctx: Ctx, slug: str) -> str | None:
    """Step 1: generate a claim id, write it to the log as `starting`, run `queue start-landing`, and rewrite the log
    line to `landing`. Returns the claim id, or None when the plan is no longer complete (exit 10: skip it)."""
    claim = secrets.token_hex(3)
    # The log names the claim before the queue does, so a crash in between leaves a `starting` claim a resume can
    # settle from `queue status` (landing.md, "Starting and resuming").
    set_claim_line(ctx.log, f"{slug} {claim} starting.")
    r = ctx.queue("start-landing", slug, "--claim", claim)
    if r.returncode == 10:
        set_claim_line(ctx.log, "none.")
        print(f"SKIP {slug}: no longer complete ({r.stderr.strip()})")
        return None
    if r.returncode != 0:
        raise Refusal(f"start-landing exit {r.returncode}: {r.stderr.strip()} (settle per plans/.agents/queue.md)")
    set_claim_line(ctx.log, f"{slug} {claim} (landing; claim commit {r.stdout.strip()[:8]}). "
                   "PRs merged so far: see History.")
    print(f"CLAIM {slug} {claim}", flush=True)
    return claim


def land_and_verify(ctx: Ctx, rd: pathlib.Path, slug: str, claim: str, pairs: list[tuple[str, str]]) -> None:
    """Step 5 for a claimed plan, then a fetch and the comparison of main with the tested tree."""
    try:
        land_plan(ctx, slug, claim, pairs)
    except Refusal as err:
        raise Refusal(f"{err}\nlanding stopped for {slug}; claim {claim} still held") from err
    ctx.run(["jj", "-R", str(ctx.checkout), *FETCH], check=False)
    # Every PR has merged by now, so a comparison that cannot run (no tested_commits.txt, say) is reported and the
    # landing still counts as done; `verify` can repeat it.
    try:
        verify_landed(ctx, rd, pairs[-1][1])
    except (Refusal, OSError) as err:
        print(f"comparison with the tested tree did not run: {err}")
    print(f"MERGED {slug} claim={claim} prs={','.join(pr for pr, _ in pairs)}")


def cmd_claim(ctx: Ctx, args) -> int:
    """Step 1 alone, for a round that claims a plan before rebasing and checking it, as landing.md's numbering does."""
    round_pairs(args.round_dir, args.slug)
    return 0 if claim_plan(ctx, args.slug) else EXIT_SKIP


def cmd_land(ctx: Ctx, args) -> None:
    """Step 5 for a plan `claim` claimed, with the claim id it printed."""
    land_and_verify(ctx, args.round_dir, args.slug, args.claim, round_pairs(args.round_dir, args.slug))


def cmd_claim_land(ctx: Ctx, args) -> int:
    """`claim` and `land` back to back, for a round whose rebase and checks already ran on the stacked plans."""
    pairs = round_pairs(args.round_dir, args.slug)
    claim = claim_plan(ctx, args.slug)
    if claim is None:
        return EXIT_SKIP
    land_and_verify(ctx, args.round_dir, args.slug, claim, pairs)
    return 0


def cmd_verify(ctx: Ctx, args) -> None:
    """`verify_landed` on its own, for a plan landed by hand or a comparison repeated after a later fetch."""
    verify_landed(ctx, args.round_dir, args.top)


def cmd_record(ctx: Ctx, args) -> None:
    """Step 7: write `recording` to the log, run `queue landed` with `notes/<slug>.final.md`, then clear the claim.
    On failure the claim line keeps `recording`, so a resume can settle it from `queue history`."""
    notes = args.round_dir / "notes" / f"{args.slug}.final.md"
    if not notes.is_file():
        raise Refusal(f"{notes} does not exist; run assemble-notes first")
    set_claim_line(ctx.log, f"{args.slug} {args.claim} recording. PRs merged: {args.merged}.")
    r = ctx.queue("landed", args.slug, "--claim", args.claim, "--merged", args.merged, "--notes", str(notes))
    if r.returncode != 0:
        raise Refusal(f"landed exit {r.returncode}: {r.stdout.strip()} {r.stderr.strip()}")
    commit = r.stdout.strip()[:8]
    set_claim_line(ctx.log, "none.")
    append_history(ctx.log, f"{args.slug} recorded ({commit}); merged {args.merged}.", refresh_status=False)
    print(f"RECORDED {args.slug} {commit}")


# --- Landing notes --------------------------------------------------------------------------------------------------

PLACEHOLDER = "CHECKS_PLACEHOLDER"


def fill_checks(body: str, checks: str) -> str:
    """A plan's notes with the round's shared checks paragraph in place of its one placeholder."""
    if body.count(PLACEHOLDER) != 1:
        raise Refusal(f"{PLACEHOLDER} missing or repeated")
    return body.replace(PLACEHOLDER, checks.rstrip("\n"))


def long_lines(text: str) -> list[int]:
    """Line numbers over 120 characters. A line that is a single code span is allowed: dprint never breaks inside a
    code span, and `dprint check` accepts such a line, as it does in the reports themselves."""
    return [i + 1 for i, line in enumerate(text.splitlines())
            if len(line) > 120 and not (line.strip().startswith("`") and line.strip().rstrip(".,;").endswith("`"))]


def cmd_assemble_notes(ctx: Ctx, args) -> None:
    """Write `notes/<slug>.final.md` from `notes/<slug>.md` and the shared `notes/_checks.md`, rewrapped with the
    repository's dprint settings. The queue script inserts the notes as written, so a line the wrap cannot fix is
    refused here rather than landing on main."""
    notes = args.round_dir / "notes"
    out = fill_checks((notes / f"{args.slug}.md").read_text(encoding="utf-8"),
                      (notes / "_checks.md").read_text(encoding="utf-8"))
    out = ctx.run(["dprint", "fmt", "--stdin", "landing-notes.md"], stdin=out).stdout
    if bad := long_lines(out):
        raise Refusal(f"lines over 120 characters: {bad}")
    final = notes / f"{args.slug}.final.md"
    final.write_text(out, encoding="utf-8")
    print(final)


# --- Round end ------------------------------------------------------------------------------------------------------


def cmd_round_end(ctx: Ctx, args) -> None:
    """Back to a clean main, abandon the local copies of the round's landed PR heads, and show what the next step
    needs: `jj status`, the checkout's size (landing.md, "Waiting between rounds", prunes over 50 GB) and the queue
    status whose head is the next watcher's baseline.

    The landed heads stay visible after a fetch (see FETCH). Only heads GitHub shows as the head of a MERGED round PR
    are abandoned, and never an ancestor of the working copy. A round PR that is not merged refuses the whole step
    unless `--skip-unmerged` says the round left it open on purpose (a blocked plan); its head is then kept.
    """
    ctx.fetch()
    ctx.jj("new", "main@origin")
    prs = args.round_dir / "prs.txt"
    heads = []
    for _slug, pairs in (read_pairs(prs).items() if prs.exists() else []):
        for pr, _bm in pairs:
            info = ctx.gh_json("pr", "view", pr, "-R", ctx.repo, "--json", "state,headRefOid")
            if info["state"] != "MERGED":
                if not args.skip_unmerged:
                    raise Refusal(f"#{pr} is {info['state']}, refusing (pass --skip-unmerged to keep its head)")
                print(f"#{pr} is {info['state']}: its head is kept")
                continue
            heads.append(info["headRefOid"])
    if heads:
        revset = "(" + " | ".join(f"present({h})" for h in heads) + ") ~ ::@"
        listed = ctx.jj("log", "--no-graph", "-r", revset, "-T", 'commit_id.short() ++ "\\n"').split()
        print(f"{len(heads)} landed heads; {len(listed)} present locally")
        if listed:
            ctx.jj("abandon", revset)
        print("abandoned", len(listed))
    print(ctx.jj("status"), end="")
    print(ctx.run(["du", "-sh", str(ctx.checkout)]).stdout, end="")
    r = ctx.queue("status")
    print(r.stdout, end="")
    if r.returncode != 0:
        raise Refusal(f"queue status failed ({r.returncode}): {r.stderr.strip()[-400:]}")


# --- Log, TODO PRs and review helpers -------------------------------------------------------------------------------


def cmd_log(ctx: Ctx, _args) -> None:
    """Append stdin as one History entry and refresh the Status line with the same stamp."""
    print(append_history(ctx.log, sys.stdin.read()))


def cmd_land_docs_prs(ctx: Ctx, args) -> None:
    """Describe, open and land single-commit PRs one at a time with jjstack: for each `bookmark=message-file`, set the
    commit's description, `submit` (push and open a draft PR), `land --plan-only`, then `land --expect <pr>
    --allow-no-checks` (CI runs only on demand), and return to main. Stops at the first failure.

    For the PRs the instructions say to land without waiting for checks: the TODO-only PR of answered blocks
    (landing.md, "Answering blocks", step 4) and others the maintainer has said to merge without asking. Whether a PR
    qualifies is the caller's call, not this command's.
    """
    jjstack = str(args.jjstack)
    for pair in args.pairs:
        bookmark, sep, msgfile = pair.partition("=")
        if not sep:
            raise Refusal(f"expected bookmark=message-file, got {pair!r}")
        ctx.fetch()
        ctx.run(["jj", "-R", str(ctx.checkout), "describe", "-r", bookmark, "--stdin"],
                stdin=pathlib.Path(msgfile).read_text(encoding="utf-8"))
        ctx.jj("new", bookmark)
        out = ctx.run([jjstack, "submit", "--dir", str(ctx.checkout), "--repo", ctx.repo])
        m = re.search(r"JJSTACK_SUBMITTED pr=(\d+)", out.stdout + out.stderr)
        if not m:
            raise Refusal(f"submit of {bookmark} printed no JJSTACK_SUBMITTED line")
        pr = m.group(1)
        ctx.run([jjstack, "land", "--dir", str(ctx.checkout), "--repo", ctx.repo, "--plan-only"])
        out = ctx.run([jjstack, "land", "--dir", str(ctx.checkout), "--repo", ctx.repo, "--expect", pr,
                       "--allow-no-checks"])
        text = out.stdout + out.stderr
        landed = [line for line in text.splitlines() if line.startswith("JJSTACK_LANDED")]
        print(bookmark, pr, landed[0] if landed else text[-300:])
        ctx.fetch()
        ctx.jj("new", "main@origin")
    print("DONE")


def cmd_approve(ctx: Ctx, args) -> None:
    """`queue approve` for each slug in order (reviewing.md), each from a freshly fetched origin/main, stopping at the
    first failure so nothing is approved twice or skipped silently. Exit 10 (no longer awaiting review: someone else
    approved or followed it up) is reported and the run goes on.

    The checkout must be a plain git clone of the reviewer's own, since this detaches its HEAD; a jj checkout (such as
    an executor's or the monitor's) is refused.
    """
    if (ctx.checkout / ".jj").exists():
        raise Refusal("approve runs in a plain git clone of the reviewer's own, not a jj checkout")
    for slug in args.slugs:
        ctx.run(["git", "-C", str(ctx.checkout), "fetch", "-q", "origin", "main"])
        ctx.run(["git", "-C", str(ctx.checkout), "checkout", "-q", "--detach", "origin/main"])
        r = ctx.queue("approve", slug)
        print(slug, "exit", r.returncode, r.stdout.strip()[-120:], r.stderr.strip()[-200:])
        if r.returncode not in (0, 10):
            raise Refusal(f"stopped at {slug}")
    print("DONE")


def review_page(plans: list[dict], reports: dict[str, str], repo: str, status: str, total: str, note: str) -> str:
    """The review page's body fragment (reviewing.md, "Reviewing the plans"): per plan its PRs and summary, what was
    done, the leftovers ("Decision needed:" marked), the full report collapsed, and an approve picker keyed by slug.

    `plans` is what the reading sub-agents wrote: `{slug, prs, summary, done, leftovers: [{text, decision_needed}]}`.
    Every string from a report or a summary is HTML-escaped; only the page's own markup is not.
    """
    e = html.escape
    tldr = f"<p><strong>Landing monitor:</strong> {e(status)}</p>"
    if note:
        tldr += f"<p>{e(note)}</p>"
    parts = [
        "<h1>Landed plans to review</h1>",
        f"<p>The {len(plans)} oldest of {e(total)} landed plans awaiting review, oldest first.</p>",
        f'<div class="tldr">{tldr}'
        "<p>Pick <strong>Approve</strong> to end a plan's tracking (its work stays on main). Write in a plan's box to "
        "ask for a follow-up round, a TODO entry, or an answer; a plan you leave untouched stays for later.</p></div>",
    ]
    for p in plans:
        slug = p["slug"]
        links = ", ".join(f'<a href="https://github.com/{e(repo)}/pull/{int(n)}/changes">#{int(n)}</a>'
                          for n in p["prs"])
        parts.append(f"<h2>{e(slug)}</h2>")
        parts.append(f"<p class=\"muted\">{links} · {e(p['summary'])}</p>")
        parts.append(f"<p>{e(p['done'])}</p>")
        if p["leftovers"]:
            items = "".join(
                f"<li>{'<strong>Decision needed:</strong> ' if lo.get('decision_needed') else ''}{e(lo['text'])}</li>"
                for lo in p["leftovers"])
            parts.append(f"<p><strong>Left over:</strong></p><ul>{items}</ul>")
        else:
            parts.append("<p><strong>Left over:</strong> nothing.</p>")
        url = f"https://github.com/{e(repo)}/blob/main/plans/reports/{e(slug)}.report.md"
        parts.append(f'<details><summary>Full report (<a href="{url}">on GitHub</a>)</summary>'
                     f"<pre>{e(reports[slug])}</pre></details>")
        parts.append(f'<div data-pick="{e(slug)}" data-label="{e(slug)}" '
                     'data-placeholder="Comment or instruction: follow-up, TODO entry, question">'
                     '<span data-value="approve">Approve</span></div>')
        parts.append("<hr>")
    return "\n".join(parts) + "\n"


def cmd_review_page(ctx: Ctx, args) -> None:
    """Write the review page fragment, reading each report from the checkout (the reviewer's fresh clone of main)."""
    plans = []
    for part in args.parts:
        plans += json.loads(pathlib.Path(part).read_text(encoding="utf-8"))
    reports = {p["slug"]: (ctx.checkout / "plans/reports" / f"{p['slug']}.report.md").read_text(encoding="utf-8")
               for p in plans}
    args.out.write_text(review_page(plans, reports, ctx.repo, args.monitor_status, args.total, args.note),
                        encoding="utf-8")
    print(args.out, len(plans))


# --- Command line ---------------------------------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--checkout", type=pathlib.Path, default=SCRIPT_CHECKOUT,
                        help="the checkout to work in (default: the one holding this script)")
    parser.add_argument("--repo", help="OWNER/NAME (default: from the checkout's origin remote)")
    sub = parser.add_subparsers(dest="command", required=True, metavar="COMMAND")
    round_dir = argparse.ArgumentParser(add_help=False)
    round_dir.add_argument("--round-dir", type=pathlib.Path, required=True, help="this round's scratch directory")

    def add(name: str, func, help: str, parents=(round_dir,)) -> argparse.ArgumentParser:
        p = sub.add_parser(name, help=help, parents=list(parents))
        p.set_defaults(func=func)
        return p

    add("round-start", cmd_round_start, "create the round directory and fix the round's [complete] plans")
    add("bases", cmd_bases, "each plan's base, main's landed work since it, and any plans/ touch")
    add("stack", cmd_stack, "stack the plans in queue order on main@origin and list conflicts")
    add("diffs", cmd_diffs, "write the cross-plan review's diffs").add_argument(
        "--oldest-base", required=True, help="the oldest plan base, as `bases` prints it")
    add("pr-map", cmd_pr_map, "write prs.txt: each bookmark's one open PR")
    add("tested", cmd_tested, "write tested_commits.txt as the combined checks start")
    add("claim", cmd_claim, "step 1: claim one plan for landing").add_argument("slug")
    p = add("land", cmd_land, "step 5: land a claimed plan's PRs bottom-up, then compare main")
    p.add_argument("slug")
    p.add_argument("claim")
    add("claim-land", cmd_claim_land, "claim and land one plan back to back").add_argument("slug")
    add("verify", cmd_verify, "compare main with the tested tree at a top bookmark").add_argument("top")
    p = add("record", cmd_record, "run queue landed with the plan's final notes and clear the claim")
    p.add_argument("slug")
    p.add_argument("claim")
    p.add_argument("merged", help="comma-separated PR numbers that merged")
    add("assemble-notes", cmd_assemble_notes, "fill the shared checks into a plan's notes and rewrap").add_argument(
        "slug")
    add("round-end", cmd_round_end, "back to main, abandon landed heads, show size and status").add_argument(
        "--skip-unmerged", action="store_true", help="keep the heads of round PRs that did not merge")
    add("log", cmd_log, "append stdin to the monitor log's History and refresh its Status", parents=())
    p = add("land-docs-prs", cmd_land_docs_prs, "open and land single-commit PRs that need no checks", parents=())
    p.add_argument("--jjstack", type=pathlib.Path, required=True, help="the jjstack skill's scripts/jjstack")
    p.add_argument("pairs", nargs="+", metavar="BOOKMARK=MESSAGE_FILE")
    add("approve", cmd_approve, "queue approve each slug from a fresh origin/main", parents=()).add_argument(
        "slugs", nargs="+", metavar="SLUG")
    p = add("review-page", cmd_review_page, "build the review page's body fragment", parents=())
    p.add_argument("--out", type=pathlib.Path, required=True)
    p.add_argument("--monitor-status", required=True, help="the monitor log's Status line")
    p.add_argument("--total", required=True, help="how many landed plans await review in all")
    p.add_argument("--note", default="", help="one more sentence for the page's summary box")
    p.add_argument("parts", nargs="+", metavar="SUMMARIES_JSON")
    return parser


def main(argv: list[str]) -> int:
    args = build_parser().parse_args(argv)
    try:
        checkout = args.checkout.resolve()
        ctx = Ctx(checkout, args.repo or default_repo(checkout))
        return args.func(ctx, args) or 0
    except Refusal as err:
        print(err, file=sys.stderr)
        return EXIT_FAIL


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
