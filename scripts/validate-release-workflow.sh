#!/usr/bin/env bash
# Parse release configuration on Linux before either packaging job spends
# runner time. This checks ownership and job-local semantics, not coincidental
# strings that could have moved to an unrelated job.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
workflow="$repo/.github/workflows/release.yml"
supervisor_unit="$repo/release/farhelm-supervisor.service.in"
helm_unit="$repo/release/farhelm-helm.service"
ziglang_requirements="$repo/.github/release/ziglang-requirements.txt"

for path in \
  "$repo/scripts/build-private-tmux.sh" \
  "$repo/.github/release/source-pins.env" \
  "$repo/.github/release/ziglang-requirements.txt" \
  "$supervisor_unit" \
  "$helm_unit" \
  "$repo/crates/farhelm-ui/Dioxus.toml"; do
  test -f "$path"
done

grep -Fqx 'ziglang==0.14.1 \' "$ziglang_requirements"
grep -Fqx '    --hash=sha256:6eb9d4d759b292c83810dbee2e9e8e3fbfbf01d864e6e9811bae711fd74e1c2f' "$ziglang_requirements"

validator=$(mktemp)
trap 'rm -f "$validator"' EXIT
cat >"$validator" <<'PY'
import configparser
import pathlib
import re
import sys
import tomllib

import yaml

workflow_path, supervisor_path, helm_path, dioxus_path = map(pathlib.Path, sys.argv[1:])
workflow = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
jobs = workflow["jobs"]

assert workflow["permissions"] == {"contents": "read"}
assert set(jobs) == {
    "validate", "linux", "publish-linux", "macos", "publish-macos"
}

publication_jobs = {"publish-linux", "publish-macos"}
for name, job in jobs.items():
    expected = {"contents": "write"} if name in publication_jobs else None
    assert job.get("permissions") == expected, (name, job.get("permissions"))
    checkouts = [step for step in job["steps"] if str(step.get("uses", "")).startswith("actions/checkout@")]
    assert len(checkouts) == 1, name
    checkout = checkouts[0]
    assert checkout["with"]["ref"] == "refs/tags/${{ env.RELEASE_TAG }}", name
    assert checkout["with"]["persist-credentials"] is False, name
    verify = [step for step in job["steps"] if step.get("name") == "Verify release source"]
    assert len(verify) == 1 and "git rev-list -n 1" in verify[0]["run"], name
    for step in job["steps"]:
        action = step.get("uses")
        if action:
            assert re.fullmatch(r"[^@]+@[0-9a-f]{40}", action), (name, action)

assert jobs["linux"]["needs"] == "validate"
assert set(jobs["macos"]["needs"]) == {"validate", "linux"}
assert jobs["macos"]["if"] == "github.event_name == 'workflow_dispatch'"
# Per-attempt cap; the 180-minute total is tracked outside CI by the
# dispatching operator.
assert jobs["macos"]["timeout-minutes"] == 60
assert jobs["publish-linux"]["needs"] == "linux"
assert jobs["publish-macos"]["needs"] == "macos"
assert jobs["publish-macos"]["if"] == "github.event_name == 'workflow_dispatch'"

def joined_steps(job_name):
    return "\n".join(
        str(value)
        for step in jobs[job_name]["steps"]
        for value in (step.get("uses", ""), step.get("run", ""), step.get("with", ""))
    )

def artifact_names(job_name, action):
    return {
        step.get("with", {}).get("name")
        for step in jobs[job_name]["steps"]
        if str(step.get("uses", "")).startswith(action + "@")
    }

linux = joined_steps("linux")
macos = joined_steps("macos")
publish_linux = joined_steps("publish-linux")
publish_macos = joined_steps("publish-macos")
validate = joined_steps("validate")
assert "pyyaml==6.0.2" in validate.lower()
assert "hashFiles('**/Cargo.lock')" in linux
assert "hashFiles('**/Cargo.lock')" in macos
assert ".github/release/source-pins.env" in linux
assert "scripts/build-private-tmux.sh" in linux
# The Mac app requires Homebrew's tmux and bundles none (TODO.md's 2026-08-22
# floor decision); a darwin private build reappearing in the macos job would
# be a silent reversal of that policy, so its every trace is forbidden here.
for forbidden in ("scripts/build-private-tmux.sh", "Contents/MacOS/tmux", "macos-14-arm64-clang"):
    assert forbidden not in macos, forbidden
assert "ziglang-0.14.1" in linux
assert "--require-hashes" in linux
assert ".github/release/ziglang-requirements.txt" in linux
linux_steps = {step.get("name"): step for step in jobs["linux"]["steps"]}
elf = linux_steps["Verify Linux payload architecture and static linkage"]["run"]
assert "readelf -h" in elf and "readelf -l" in elf and "file \"$payload\"" in elf
assert "x86_64" in elf and "aarch64" in elf
assert artifact_names("linux", "actions/upload-artifact") == {
    "release-inputs", "linux-release-assets"
}
assert artifact_names("macos", "actions/upload-artifact") == {"macos-release-assets"}
assert "gh release upload" not in linux + macos
assert artifact_names("publish-linux", "actions/download-artifact") == {"linux-release-assets"}
assert "gh release upload" in publish_linux
assert artifact_names("publish-macos", "actions/download-artifact") == {"macos-release-assets"}
assert "gh release upload" in publish_macos

def unit(path):
    # strict=False because systemd allows a directive to repeat and the
    # supervisor unit uses two Environment= lines; configparser's
    # one-value-per-key model would raise on the second. Repeated
    # directives are checked by `directives` below instead.
    parser = configparser.RawConfigParser(strict=False)
    parser.optionxform = str
    parser.read(path, encoding="utf-8")
    return parser


def directives(path, name):
    """Every value of a possibly-repeated unit directive, in file order."""
    prefix = f"{name}="
    return [
        line[len(prefix):]
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.startswith(prefix)
    ]

supervisor = unit(supervisor_path)
assert "After" not in supervisor["Unit"]
assert supervisor["Service"]["Type"] == "simple"
assert supervisor["Service"]["ExecStart"] == "@FARHELM@ supervisor run --state-dir @STATE_DIR@"
# Both environment lines, in order. FARHELM_TMUX is what names the exact
# tmux the supervisor drives; PATH alone let a stale private tmux shadow
# the binary provisioning had accepted.
assert directives(supervisor_path, "Environment") == [
    '"PATH=@PATH@"',
    '"FARHELM_TMUX=@TMUX@"',
]
assert supervisor["Service"]["KillMode"] == "process"
assert supervisor["Service"]["Restart"] == "on-failure"
assert supervisor["Install"]["WantedBy"] == "default.target"

helm = unit(helm_path)
assert "After" not in helm["Unit"]
assert helm["Service"]["ExecStart"] == "%h/.local/lib/farhelm/farhelm helm run --ui-dist %h/.local/lib/farhelm/web"
assert helm["Service"]["Restart"] == "on-failure"
assert helm["Install"]["WantedBy"] == "default.target"

with dioxus_path.open("rb") as source:
    dioxus = tomllib.load(source)
assert dioxus["bundle"]["macos"]["bundle_name"] == "Farhelm"
PY

if command -v uv >/dev/null 2>&1; then
  uv run --quiet --with pyyaml==6.0.2 python "$validator" "$workflow" "$supervisor_unit" "$helm_unit" "$repo/crates/farhelm-ui/Dioxus.toml"
else
  python3 "$validator" "$workflow" "$supervisor_unit" "$helm_unit" "$repo/crates/farhelm-ui/Dioxus.toml"
fi

source "$repo/.github/release/source-pins.env"
for checksum in "$TMUX_SHA256" "$LIBEVENT_SHA256" "$NCURSES_SHA256"; do
  [[ "$checksum" =~ ^[0-9a-f]{64}$ ]]
done
