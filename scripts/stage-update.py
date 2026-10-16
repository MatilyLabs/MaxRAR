#!/usr/bin/env python3
"""Stage signed updater assets locally. Does not create or publish a GitHub release."""
import argparse
import base64
import json
import pathlib
import re
import shutil
import urllib.parse

SUFFIXES = {'darwin-aarch64': '.app.tar.gz', 'darwin-x86_64': '.app.tar.gz',
            'windows-x86_64': '.exe', 'linux-x86_64': '.AppImage'}

def signature_version(signature):
    try:
        text = base64.b64decode(signature, validate=True).decode('utf-8')
        comment = next(line.removeprefix('trusted comment: ') for line in text.splitlines()
                       if line.startswith('trusted comment: '))
        return next(field.removeprefix('version:') for field in comment.split('\t')
                    if field.startswith('version:'))
    except (ValueError, UnicodeError, StopIteration) as error:
        raise ValueError('Signature must include a signed application version') from error

def stage(root, platform, version, output):
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?', version):
        raise ValueError('Version must be a semantic version without a v prefix')
    config = json.loads((pathlib.Path(__file__).resolve().parent.parent / 'release.config.json').read_text())
    candidates = [p for p in pathlib.Path(root).rglob('*' + SUFFIXES[platform])
                  if p.is_file() and pathlib.Path(str(p) + '.sig').is_file()]
    if len(candidates) != 1:
        raise ValueError(f'Expected one signed updater asset for {platform}; found {len(candidates)}')
    asset = candidates[0]
    signature = pathlib.Path(str(asset) + '.sig').read_text().strip()
    if not signature or len(signature) > 8192 or signature_version(signature) != version:
        raise ValueError('Missing or invalid signature file')
    output = pathlib.Path(output)
    output.mkdir(parents=True, exist_ok=True)
    # Include platform in the staged name so Intel/Apple-Silicon assets cannot collide.
    name = platform + '-' + asset.name
    shutil.copy2(asset, output / name)
    shutil.copy2(str(asset) + '.sig', output / (name + '.sig'))
    # Initial installations and updater downloads come from the same build.
    extras = ['*.dmg'] if platform.startswith('darwin-') else ['*.deb', '*.rpm'] if platform.startswith('linux-') else []
    for pattern in extras:
        for installer in pathlib.Path(root).rglob(pattern):
            shutil.copy2(installer, output / installer.name)
    url = config['repository'] + '/releases/download/v' + version + '/' + urllib.parse.quote(name)
    manifest = {'version': version, 'notes': '', 'platforms': {platform: {'url': url, 'signature': signature}}}
    (output / (platform + '.json')).write_text(json.dumps(manifest, indent=2) + '\n')
    return manifest

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True)
    parser.add_argument('--platform', required=True, choices=SUFFIXES)
    parser.add_argument('--version', required=True)
    parser.add_argument('--output', default='release-stage')
    args = parser.parse_args()
    stage(args.root, args.platform, args.version, args.output)
