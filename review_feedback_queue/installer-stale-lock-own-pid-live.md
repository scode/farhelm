# A stale lock with the installer's own pid looks live

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In a container or scripted environment, one interrupted install can make every later install fail with a false "already
running" message until the user deletes the lock directory by hand.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F9 / COR-STALE-LOCK-OWN-PID`, tagged **possible**. Anchors and title: `scripts/install.sh:694` — A stale lock whose pid
equals the current shell's pid is treated as live on every run

`acquire_lock` decides whether an existing lock is live by running `kill -0 "$other_pid"` on the pid stored in it
(L694). It never excludes its own pid, `$$`, and `kill -0 $$` always succeeds. In containers and other deterministic
launch environments, `sh` often gets the same small pid on every start. If an install there was killed by SIGKILL or a
power loss (which skip the exit handler and leave the lock behind), and the install directory survives (a volume, or a
restarted container), every later run finds its own pid in the lock and refuses with "another farhelm install/update
(pid N) is already running". Journal recovery never runs, and the only way out is deleting the lock directory by hand.

The script's comments already accept that `kill -0` can be fooled by an unrelated process reusing the pid, as a narrow
race. Reuse by the _current_ process is different: it can be deterministic, and it is the one case the script can
recognise with certainty. Suggested change: treat `other_pid = $$` as stale.
