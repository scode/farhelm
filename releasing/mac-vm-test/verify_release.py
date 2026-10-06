#!/usr/bin/env python3
"""Verify a published installer against the previous release's compiled trust ring.

Run inside the Mac guest's source checkout. This downloads verification inputs
and the installer only; it never installs anything. Separate verification is
useful evidence, but does not substitute for exercising the old app's updater.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
from urllib.request import urlopen


def code_mask(source):
    """Preserve code offsets while hiding Rust comments and string contents.

Declaration lookup must not find historical declarations in comments or quoted
examples. Masking preserves offsets so the actual array's original literals can
still be parsed without corrupting base64 keys containing comment-like slashes.
Nested block comments and raw strings need their own lexical boundaries.
"""
    masked = list(source)
    raw_token = re.compile(r'(?:br|r)(#*)"')
    character_token = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'")
    offset = 0
    while offset < len(source):
        start = offset
        if source.startswith('//', offset):
            end = source.find('\n', offset)
            offset = len(source) if end < 0 else end
        elif source.startswith('/*', offset):
            depth = 1
            offset += 2
            while depth and offset < len(source):
                if source.startswith('/*', offset):
                    depth += 1
                    offset += 2
                elif source.startswith('*/', offset):
                    depth -= 1
                    offset += 2
                else:
                    offset += 1
            if depth:
                raise ValueError('unterminated Rust block comment')
        else:
            raw = raw_token.match(source, offset)
            if raw:
                end = source.find('"' + raw.group(1), raw.end())
                if end < 0:
                    raise ValueError('unterminated Rust raw string')
                offset = end + 1 + len(raw.group(1))
            elif source[offset] == '"':
                offset += 1
                while offset < len(source) and source[offset] != '"':
                    offset += 2 if source[offset] == '\\' else 1
                if offset >= len(source):
                    raise ValueError('unterminated Rust string')
                offset += 1
            else:
                # Rust lifetimes are not character literals. Recognize only a
                # closed character token so 'static cannot hide later code.
                character = character_token.match(source, offset)
                if not character:
                    offset += 1
                    continue
                offset = character.end()
        masked[start:offset] = ['\n' if char == '\n' else ' ' for char in source[start:offset]]
    return ''.join(masked)


def key_ring(source):
    """Read the recognized literal key ring without trusting commented entries.

The previous tag is the authority. Main or the candidate may already trust a
new key that the installed old app would refuse. This accepts the release's
literal string-array layout, not arbitrary Rust expressions; an unfamiliar
layout must be inspected rather than guessed into a broader trust ring. Locate
exactly one active declaration outside comments and quoted examples; ambiguous
source cannot establish which keys the old binary compiled.
"""
    matches = list(re.finditer(r'pub const RELEASE_KEY_RING:[^=]*=\s*&\[(.*?)\];', code_mask(source), re.DOTALL))
    if len(matches) != 1:
        raise ValueError('previous source needs exactly one recognized active key ring declaration')
    start, end = matches[0].span(1)
    body = source[start:end]
    token = re.compile(r'\s+|,|//[^\n]*(?:\n|$)|/\*(?:(?!/\*).)*?\*/|"([A-Za-z0-9+/=]+)"', re.DOTALL)
    keys = []
    offset = 0
    while offset < len(body):
        entry = token.match(body, offset)
        if not entry:
            raise ValueError('previous key ring has unsupported array syntax')
        if entry.group(1):
            keys.append(entry.group(1))
        offset = entry.end()
    if not keys:
        raise ValueError('previous compiled key ring is empty')
    return keys


def require_comment(signature, version):
    """Require the signed version binding exactly; an extra v is not equivalent."""
    comments = [line for line in signature.splitlines() if line.startswith('trusted comment:')]
    if comments != [f'trusted comment: farhelm {version}']:
        raise ValueError('signature trusted comment does not bind the requested version')


def verify(previous, version, output):
    """Retain the signed inputs and refuse before executing any downloaded bytes."""
    for tag in (previous, version):
        if not re.fullmatch(r'v[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?', tag):
            raise ValueError('provide a release tag, not an arbitrary URL or path')
    source = subprocess.check_output(['git', 'show',
        f'{previous}:crates/farhelm-helm/src/provisioning/release_payloads.rs'], text=True)
    keys = key_ring(source)
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    for name in ('SHA256SUMS', 'SHA256SUMS.minisig', 'install.sh'):
        with urlopen(f'https://get.farhelm.io/{version}/{name}', timeout=30) as response:
            content = response.read(1024 * 1024 + 1)
        if len(content) > 1024 * 1024:
            raise ValueError(f'{name} exceeds verification input limit')
        (output / name).write_bytes(content)
        (output / name).chmod(0o600)
    require_comment((output / 'SHA256SUMS.minisig').read_text(), version)
    accepted = None
    for index, key in enumerate(keys):
        result = subprocess.run(['minisign', '-V', '-P', key, '-m', str(output / 'SHA256SUMS'),
                                 '-x', str(output / 'SHA256SUMS.minisig')], capture_output=True, timeout=10)
        if result.returncode == 0:
            accepted = index
            break
    if accepted is None:
        raise ValueError('no key in the previous release ring verifies the signature')
    sums = {}
    for line in (output / 'SHA256SUMS').read_text().splitlines():
        fields = line.split()
        if len(fields) != 2 or not re.fullmatch(r'[a-fA-F0-9]{64}', fields[0]) or fields[1] in sums:
            raise ValueError('malformed or duplicate signed checksum entry')
        sums[fields[1]] = fields[0].lower()
    installer_hash = hashlib.sha256((output / 'install.sh').read_bytes()).hexdigest()
    if sums.get('install.sh') != installer_hash:
        raise ValueError('installer does not match its signed checksum')
    report = {'version': version, 'previous_trust_ring': previous, 'verified_key_index': accepted,
              'sha256sums_sha256': hashlib.sha256((output / 'SHA256SUMS').read_bytes()).hexdigest(),
              'installer_sha256': installer_hash, 'signature': 'verified', 'trusted_comment': 'verified',
              'installer_checksum': 'verified'}
    (output / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


def main():
    """Keep tags and the guest evidence destination explicit for each artifact pair."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--previous', required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    verify(args.previous, args.version, args.output)


if __name__ == '__main__':
    main()
