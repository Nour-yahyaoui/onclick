#!/usr/bin/env python3
"""Bump OneClick's version in every manifest at once.

Usage:
    python3 scripts/bump_version.py            # 0.5.6 -> 0.5.7 (patch)
    python3 scripts/bump_version.py minor      # 0.5.7 -> 0.6.0
    python3 scripts/bump_version.py 0.7.2      # explicit

Rule for this repo: every commit bumps the version, so a shop can tell exactly
which build it is running from the sign-in screen and from `store --version`.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MANIFESTS = [
    ROOT / 'Cargo.toml',
    ROOT / 'crates/domain/Cargo.toml',
    ROOT / 'crates/storage/Cargo.toml',
    ROOT / 'crates/ui/Cargo.toml',
    ROOT / 'crates/app/Cargo.toml',
    ROOT / 'crates/control/Cargo.toml',
]


def current() -> str:
    text = (ROOT / 'Cargo.toml').read_text()
    match = re.search(r'^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"', text, re.M)
    if not match:
        sys.exit('no version found in Cargo.toml')
    return match.group(1)


def next_version(old: str, bump: str) -> str:
    major, minor, patch = (int(p) for p in old.split('.'))
    if re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', bump):
        return bump
    if bump == 'minor':
        return f'{major}.{minor + 1}.0'
    if bump == 'major':
        return f'{major + 1}.0.0'
    return f'{major}.{minor}.{patch + 1}'


def main() -> int:
    old = current()
    new = next_version(old, sys.argv[1] if len(sys.argv) > 1 else 'patch')
    for manifest in MANIFESTS:
        if not manifest.exists():
            continue
        text = manifest.read_text()
        updated = re.sub(
            r'^version\s*=\s*"[0-9]+\.[0-9]+\.[0-9]+"',
            f'version = "{new}"',
            text,
            count=1,
            flags=re.M,
        )
        manifest.write_text(updated)
        print(f'{manifest.relative_to(ROOT)} -> {new}')
    print(f'\n{old} -> {new}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
