#!/usr/bin/env python3
"""Require an existing version tag at the exact build commit before release builds."""
import argparse
import json
import base64
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def check(repository, commit):
    version = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())['version']
    tag = 'v' + version
    result = subprocess.run(['gh', 'api', f'repos/{repository}/commits/{tag}'], capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(f'Push the {tag} version tag before starting release builds. No builds have started.')
    if json.loads(result.stdout)['sha'] != commit:
        raise SystemExit(f'{tag} does not point to this workflow commit. Dispatch the workflow on {tag}.')
    result = subprocess.run(['gh', 'release', 'view', tag, '--repo', repository, '--json', 'isDraft'], capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(f'Prepare the {tag} draft release with your maintainer account before building.')
    if not json.loads(result.stdout)['isDraft']:
        raise SystemExit(f'{tag} is already published; refusing to replace its packages.')
    print(f'{tag}: existing tag matches {commit} and its draft is ready.')


def verified_source(repository, run_id):
    if not re.fullmatch(r'[0-9]+', run_id):
        raise SystemExit('Source run must be a numeric GitHub Actions run ID.')
    def api(path):
        return json.loads(subprocess.check_output(['gh', 'api', f'repos/{repository}/{path}'], text=True))
    run = api(f'actions/runs/{run_id}')
    workflow = api('actions/workflows/prepare-updates.yml')
    if run['workflow_id'] != workflow['id']:
        raise SystemExit('Source run is not the MaxRAR signed release workflow.')
    jobs = api(f'actions/runs/{run_id}/jobs?per_page=100')['jobs']
    platforms = [job for job in jobs if job['name'].startswith('Signed release')]
    if len(platforms) != 4 or any(job['conclusion'] != 'success' for job in platforms):
        raise SystemExit('Source run must contain four successful signed platform jobs.')
    commit = run['head_sha']
    source_config = api(f'contents/release.config.json?ref={commit}')
    source_config = json.loads(base64.b64decode(source_config['content']))
    current_config = json.loads((ROOT / 'release.config.json').read_text())
    if source_config != current_config:
        raise SystemExit('Source signing key or release configuration differs; do not mix release artifacts.')
    return commit


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--source-run', default='')
    parser.add_argument('--current-run', default='')
    parser.add_argument('--output')
    args = parser.parse_args()
    commit = verified_source(args.repository, args.source_run) if args.source_run else args.commit
    check(args.repository, commit)
    if args.output:
        run_id = args.source_run or args.current_run
        if not re.fullmatch(r'[0-9]+', run_id):
            raise SystemExit('Missing current workflow run ID.')
        with Path(args.output).open('a') as output:
            output.write(f'run_id={run_id}\n')
