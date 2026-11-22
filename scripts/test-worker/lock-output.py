"""Give one driver ownership of the local evidence directory through exec and exit.

Lock the directory itself to identify aliases of the same destination and avoid
leaving an artifact or a stale lock after the driver is interrupted. The driver
closes fd 9 when it starts a transport that can outlive it.
"""

import fcntl
import os
from pathlib import Path
import sys

out, driver, *args = sys.argv[1:]
try:
    Path(out).mkdir(parents=True, exist_ok=True)
    fd = os.open(out, os.O_RDONLY | os.O_DIRECTORY)
    fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
except BlockingIOError:
    print(f"test-worker: output directory {out} is in use", file=sys.stderr)
    sys.exit(75)
except OSError as error:
    sys.exit(f"test-worker: could not open output directory {out}: {error}")

os.dup2(fd, 9)
os.set_inheritable(9, True)
if fd != 9:
    os.close(fd)
os.execv("/bin/bash", ["bash", driver, "__output_locked", *args])
