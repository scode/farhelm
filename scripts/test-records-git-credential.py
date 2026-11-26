"""Tests for records-git-credential.py, run as a child process the way git runs it.

The helper is the only thing standing between an attended clone and its repo token, so these pin the contract git and
Cloudflare depend on: the Basic-auth answer strips the `?expires=` suffix, nothing is offered over plain HTTP or to a
host other than an Artifacts git host, `store` and `erase` never touch the token file, and failures say so without
printing a password. The last case makes a clone with the exact `git clone -c ...` command attended flows use and asks
that clone's git for a credential, which proves the "absolute path plus token file argument" form survives git's own
invocation through the shell.

The git children get an explicit environment (`env=`) isolated from the user's git config; the test process's own
environment is never changed.
"""
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().with_name("records-git-credential.py")


def helper(token_file, action, stdin):
    """Runs the helper directly (through its shebang, as git does) and returns the completed process."""
    return subprocess.run([str(SCRIPT), str(token_file), action], input=stdin, capture_output=True, text=True,
                          check=False)


class CredentialHelperTest(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="records-credential-test-"))
        self.addCleanup(shutil.rmtree, self.tmp)
        self.token_file = self.tmp / "agent-records_token"
        self.token_file.write_text("art_v1_abcdef?expires=1760000000\n")

    def test_get_answers_basic_auth_with_the_token_before_expires(self):
        """`get` over https prints any username and the token's part before `?expires=`: Cloudflare's Basic-auth
        rule. Sending the whole string as the password would fail authentication."""
        result = helper(self.token_file, "get", "protocol=https\nhost=acct.artifacts.cloudflare.net\n\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "username=x\npassword=art_v1_abcdef\n")

    def test_token_without_expiry_is_used_whole(self):
        """A token without the suffix is the password as it is."""
        self.token_file.write_text("plain-token")
        result = helper(self.token_file, "get", "protocol=https\nhost=acct.artifacts.cloudflare.net\n")
        self.assertEqual(result.stdout, "username=x\npassword=plain-token\n")

    def test_plain_http_gets_no_answer(self):
        """Over a non-https protocol the helper answers nothing, so the token is never sent in the clear."""
        result = helper(self.token_file, "get", "protocol=http\nhost=acct.artifacts.cloudflare.net\n\n")
        self.assertEqual((result.returncode, result.stdout), (0, ""))

    def test_foreign_hosts_get_no_answer(self):
        """Over https to any host that is not an Artifacts git host the helper answers nothing: the token belongs to
        one service, and a redirect or a misplaced helper entry must not hand it to another."""
        for host in ("github.com", "artifacts.cloudflare.net.example.com", "evil-artifacts.cloudflare.net.invalid",
                     "acct.artifacts.cloudflare.net:8443", ""):
            result = helper(self.token_file, "get", f"protocol=https\nhost={host}\n\n")
            self.assertEqual((result.returncode, result.stdout), (0, ""), host)

    def test_store_and_erase_leave_the_token_file_alone(self):
        """`store` and `erase` succeed silently and leave the token file intact: git erases credentials after a
        failed attempt, and the file is the only copy."""
        for action in ("store", "erase"):
            request = "protocol=https\nhost=acct.artifacts.cloudflare.net\nusername=x\npassword=p\n\n"
            result = helper(self.token_file, action, request)
            self.assertEqual((result.returncode, result.stdout), (0, ""), action)
        self.assertEqual(self.token_file.read_text(), "art_v1_abcdef?expires=1760000000\n")

    def test_unreadable_or_empty_token_file_fails_without_a_password(self):
        """A missing or empty token file exits 1 with a message naming the file, and prints no credential."""
        for setup in ("missing", "empty"):
            if setup == "missing":
                self.token_file.unlink()
            else:
                self.token_file.write_text("\n")
            result = helper(self.token_file, "get", "protocol=https\nhost=acct.artifacts.cloudflare.net\n\n")
            self.assertEqual(result.returncode, 1, setup)
            self.assertEqual(result.stdout, "", setup)
            self.assertIn(str(self.token_file), result.stderr, setup)

    def test_wrong_usage_exits_2(self):
        """No token file argument, or an unknown action, is a usage error."""
        for argv in ([str(SCRIPT), "get"], [str(SCRIPT), str(self.token_file), "fill"]):
            result = subprocess.run(argv, input="", capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 2, argv)

    @unittest.skipUnless(shutil.which("git"), "git is not installed")
    def test_attended_clone_command_configures_a_working_helper(self):
        """The documented attended clone command (`git clone -c credential.helper= -c 'credential.helper=<script>
        <token file>' <remote> <dir>`, here against a local bare repo) leaves the clone with exactly those two helper
        entries, and that clone's git then gets the stripped token for an Artifacts host and nothing for a foreign
        one. The isolated git config keeps the user's own helpers out of the answer."""
        env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null", GIT_TERMINAL_PROMPT="0")

        def git(*args, stdin=None, cwd=None):
            return subprocess.run(["git", *args], input=stdin, capture_output=True, text=True, env=env,
                                  cwd=cwd or self.tmp, check=False)

        remote = self.tmp / "remote.git"
        clone = self.tmp / "clone"
        self.assertEqual(git("init", "--bare", "-q", str(remote)).returncode, 0)
        helper_entry = f"credential.helper={SCRIPT} {self.token_file}"
        made = git("clone", "-q", "-c", "credential.helper=", "-c", helper_entry, str(remote), str(clone))
        self.assertEqual(made.returncode, 0, made.stderr)

        entries = git("config", "--local", "--get-all", "credential.helper", cwd=clone)
        self.assertEqual(entries.stdout.split("\n")[:2], ["", f"{SCRIPT} {self.token_file}"])

        request = "protocol=https\nhost=acct.artifacts.cloudflare.net\npath=git/records/agent-records.git\n\n"
        filled = git("credential", "fill", stdin=request, cwd=clone)
        self.assertEqual(filled.returncode, 0, filled.stderr)
        self.assertIn("password=art_v1_abcdef", filled.stdout.splitlines())

        # A foreign host gets no answer, and with prompting off git's fill fails instead of asking anyone.
        foreign = git("credential", "fill", stdin="protocol=https\nhost=github.com\n\n", cwd=clone)
        self.assertNotEqual(foreign.returncode, 0)
        self.assertNotIn("art_v1_abcdef", foreign.stdout)

if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]] + sys.argv[1:])
