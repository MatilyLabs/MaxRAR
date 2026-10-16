#!/usr/bin/env python3
"""Fail release preparation before expensive builds if update setup is incomplete."""
import base64
import json
import os
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def check(require_secrets=False):
    release = json.loads((ROOT / 'release.config.json').read_text())
    tauri = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
    package = json.loads((ROOT / 'package.json').read_text())
    lock = json.loads((ROOT / 'package-lock.json').read_text())
    cargo = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    version = tauri['version']
    assert version == package['version'] == lock['version'] == lock['packages']['']['version'] == cargo['workspace']['package']['version'], 'Release versions differ'
    key = release['updaterPublicKey']
    decoded = base64.b64decode(key, validate=True).decode('utf-8').splitlines()
    assert len(decoded) == 2 and decoded[0].startswith('untrusted comment:'), 'Invalid public key'
    assert len(base64.b64decode(decoded[1], validate=True)) == 42, 'Invalid public key length'
    updater = tauri['plugins']['updater']
    assert updater['pubkey'] == key, 'Updater verification keys differ'
    assert updater['requireSignedVersion'] is True, 'Signed version binding is required'
    assert release['repository'] == 'https://github.com/MatilyLabs/MaxRAR', 'Unexpected release repository'
    assert release['updaterEndpoint'] == release['repository'] + '/releases/latest/download/latest.json', 'Unexpected update endpoint'
    assert updater['endpoints'] == [release['updaterEndpoint']], 'Update endpoints differ'
    assert release['downloads'] == release['repository'] + '/releases/latest', 'Unexpected download page'
    if require_secrets:
        for name in ('TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'):
            assert os.environ.get(name), f'Missing release secret: {name}'
    print(f'MaxRAR {version}: release versions, public key and updater endpoint validated.')


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--require-secrets', action='store_true')
    check(parser.parse_args().require_secrets)
