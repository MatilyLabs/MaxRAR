#!/usr/bin/env python3
"""Combine all four verified platform artifacts into a complete release feed."""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
from urllib.parse import quote, unquote
import zipfile

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('stage_update', ROOT / 'scripts/stage-update.py')
stage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage)
PLATFORMS = set(stage.SUFFIXES)


def assemble(source, output, notes):
    source, output = Path(source), Path(output)
    assert not output.exists() or not any(output.iterdir()), 'Release output must be empty'
    config = json.loads((ROOT / 'release.config.json').read_text())
    version = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())['version']
    prefix = config['repository'] + '/releases/download/v' + version + '/'
    platforms = {}
    files = {}
    for path in sorted(source.rglob('*.json')):
        manifest = json.loads(path.read_text())
        assert manifest['version'] == version, 'Mixed release versions'
        assert len(manifest['platforms']) == 1, 'Expected one platform per staging manifest'
        platform, entry = next(iter(manifest['platforms'].items()))
        assert platform in PLATFORMS and platform not in platforms, 'Duplicate or unknown platform'
        assert entry['url'].startswith(prefix), 'Update URL must target this release'
        name = unquote(entry['url'][len(prefix):])
        assert Path(name).name == name and name.startswith(platform + '-'), 'Invalid platform asset name'
        assert entry['url'] == prefix + quote(name), 'Unexpected asset URL encoding'
        asset = path.parent / name
        assert asset.is_file(), 'Updater asset is missing'
        assert name.endswith(stage.SUFFIXES[platform]), 'Wrong updater asset type'
        assert (path.parent / (name + '.sig')).read_text().strip() == entry['signature'], 'Signature sidecar differs'
        assert stage.signature_version(entry['signature']) == version, 'Signature version differs'
        platforms[platform] = entry
        for item in path.parent.iterdir():
            if item.is_file() and item.suffix != '.json' and item.name not in ('LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES.md'):
                assert item.name not in files, 'Duplicate release asset'
                files[item.name] = item
    assert set(platforms) == PLATFORMS, 'All four platforms are required'
    assert sum(name.endswith('.dmg') for name in files) == 2, 'Both Mac installers are required'
    assert any(name.endswith('.deb') for name in files) and any(name.endswith('.rpm') for name in files), 'Linux DEB and RPM are required'
    output.mkdir(parents=True, exist_ok=True)
    for name, item in files.items():
        shutil.copy2(item, output / name)
    feed = {'version': version, 'notes': Path(notes).read_text(), 'pub_date': datetime.now(timezone.utc).isoformat(), 'platforms': platforms}
    (output / 'latest.json').write_text(json.dumps(feed, indent=2) + '\n')
    with zipfile.ZipFile(output / f'MaxRAR-{version}-Licenses.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for name in ('LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES.md'):
            archive.write(ROOT / name, name)
        for folder in ('licenses', 'design/icons/fluent', 'crates/wonrar-scan/rules'):
            for item in sorted((ROOT / folder).rglob('*')):
                if item.is_file():
                    archive.write(item, str(item.relative_to(ROOT)))
    hashes = [hashlib.sha256(item.read_bytes()).hexdigest() + '  ' + item.name for item in sorted(output.iterdir())]
    (output / 'SHA256SUMS.txt').write_text('\n'.join(hashes) + '\n')
    print(f'MaxRAR {version}: complete four-platform feed and {len(files)} package/signature assets staged.')
    return feed


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--notes', required=True)
    args = parser.parse_args()
    assemble(args.source, args.output, args.notes)
