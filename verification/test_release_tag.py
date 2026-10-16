import importlib.util
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('check_release_tag', Path(__file__).resolve().parent.parent / 'scripts/check-release-tag.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def response(code=0, data=None, error=''):
    return subprocess.CompletedProcess([], code, json.dumps(data), error)


class ReleaseTagTests(unittest.TestCase):
    def test_matching_existing_tag_can_start_a_new_release(self):
        with patch.object(module.subprocess, 'run', side_effect=[response(data={'sha': 'built-commit'}), response(data={'isDraft': True})]):
            module.check('MatilyLabs/MaxRAR', 'built-commit')

    def test_missing_or_moved_tag_stops_before_building(self):
        for result in [response(1, error='HTTP 404'), response(data={'sha': 'other-commit'})]:
            with self.subTest(result=result), patch.object(module.subprocess, 'run', return_value=result):
                with self.assertRaises(SystemExit): module.check('MatilyLabs/MaxRAR', 'built-commit')

    def test_published_release_cannot_have_its_packages_replaced(self):
        with patch.object(module.subprocess, 'run', side_effect=[response(data={'sha': 'built-commit'}), response(data={'isDraft': False})]):
            with self.assertRaises(SystemExit): module.check('MatilyLabs/MaxRAR', 'built-commit')

    def test_access_error_is_not_treated_as_absent_release(self):
        with patch.object(module.subprocess, 'run', side_effect=[response(data={'sha': 'built-commit'}), response(1, error='HTTP 403')]):
            with self.assertRaises(SystemExit): module.check('MatilyLabs/MaxRAR', 'built-commit')

    def test_missing_draft_stops_before_building(self):
        with patch.object(module.subprocess, 'run', side_effect=[response(data={'sha': 'built-commit'}), response(1, error='release not found')]):
            with self.assertRaises(SystemExit): module.check('MatilyLabs/MaxRAR', 'built-commit')

    def test_assembly_reuse_requires_four_verified_platforms_from_this_workflow(self):
        config = json.loads((module.ROOT/'release.config.json').read_text())
        import base64
        encoded = base64.b64encode(json.dumps(config).encode()).decode()
        jobs = [{'name': f'Signed release ({platform})', 'conclusion': 'success'} for platform in ('windows', 'linux', 'arm', 'intel')]
        replies = [{'workflow_id': 7, 'head_sha': 'built-commit'}, {'id': 7}, {'jobs': jobs}, {'content': encoded}]
        with patch.object(module.subprocess, 'check_output', side_effect=[json.dumps(item) for item in replies]):
            self.assertEqual(module.verified_source('MatilyLabs/MaxRAR', '123'), 'built-commit')
        jobs[0]['conclusion'] = 'failure'
        with patch.object(module.subprocess, 'check_output', side_effect=[json.dumps(item) for item in replies]):
            with self.assertRaises(SystemExit): module.verified_source('MatilyLabs/MaxRAR', '123')
        with self.assertRaises(SystemExit): module.verified_source('MatilyLabs/MaxRAR', 'not-a-run-id')


if __name__ == '__main__': unittest.main()
