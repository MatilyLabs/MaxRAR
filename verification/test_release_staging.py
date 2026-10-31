import importlib.util
import base64
import json
import pathlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('stage_update', pathlib.Path(__file__).resolve().parent.parent / 'scripts/stage-update.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def signature(version):
    # Structural fixture only; the native release verifier checks real signatures.
    return base64.b64encode(f'untrusted comment: test\nfixture\ntrusted comment: timestamp:1\tversion:{version}\nfixture\n'.encode()).decode()

class StagingTests(unittest.TestCase):
    def test_signed_assets_get_distinct_platform_names_and_encoded_urls(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            source = root / 'source'
            source.mkdir()
            asset = source / 'MaxRAR app.app.tar.gz'
            asset.write_bytes(b'fixture')
            pathlib.Path(str(asset) + '.sig').write_text(signature('0.2.0'))
            a = module.stage(source, 'darwin-aarch64', '0.2.0', root/'out-arm')
            b = module.stage(source, 'darwin-x86_64', '0.2.0', root/'out-intel')
            url = a['platforms']['darwin-aarch64']['url']
            self.assertIn('/MatilyLabs/MaxRAR/releases/download/v0.2.0/darwin-aarch64-', url)
            self.assertIn('%20', url)
            self.assertNotEqual(url, b['platforms']['darwin-x86_64']['url'])

    def test_missing_signature_and_invalid_version_cannot_be_staged(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root/'app.exe').write_bytes(b'fixture')
            with self.assertRaises(ValueError): module.stage(root,'windows-x86_64','0.2.0',root/'out')
            with self.assertRaises(ValueError): module.stage(root,'windows-x86_64','latest',root/'out')

    def test_signature_cannot_announce_another_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root/'app.exe').write_bytes(b'fixture')
            (root/'app.exe.sig').write_text(signature('0.1.0'))
            with self.assertRaises(ValueError): module.stage(root,'windows-x86_64','0.2.0',root/'out')

    def test_complete_feed_requires_all_platforms_and_matching_signature_sidecars(self):
        assemble_spec = importlib.util.spec_from_file_location('assemble_release', pathlib.Path(__file__).resolve().parent.parent / 'scripts/assemble-release.py')
        assemble = importlib.util.module_from_spec(assemble_spec)
        assemble_spec.loader.exec_module(assemble)
        version = json.loads((assemble.ROOT/'src-tauri/tauri.conf.json').read_text())['version']
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            notes = root/'notes.md'; notes.write_text('Fixture notes')
            for platform, suffix in module.SUFFIXES.items():
                source = root/'source'/platform; source.mkdir(parents=True)
                asset = source/('MaxRAR'+suffix)
                asset.write_bytes(b'fixture')
                pathlib.Path(str(asset)+'.sig').write_text(signature(version))
                if platform.startswith('darwin-'): (source/f'MaxRAR-{platform}.dmg').write_bytes(b'installer')
                if platform.startswith('linux-'):
                    for ext in ('deb','rpm'): (source/f'MaxRAR.{ext}').write_bytes(b'installer')
                module.stage(source,platform,version,root/'staged'/platform)
            feed = assemble.assemble(root/'staged',root/'out',notes)
            self.assertEqual(set(feed['platforms']),set(module.SUFFIXES))
            self.assertTrue((root/'out/SHA256SUMS.txt').is_file())
            self.assertTrue((root/f'out/MaxRAR_{version}_macOS_apple-silicon.dmg').is_file())
            self.assertTrue((root/f'out/MaxRAR_{version}_macOS_intel.dmg').is_file())
            self.assertEqual(feed['notes'],'Fixture notes')
            sidecar = next((root/'staged/windows-x86_64').glob('*.sig'))
            sidecar.write_text(signature('0.0.0'))
            with self.assertRaises(AssertionError): assemble.assemble(root/'staged',root/'bad-signature',notes)
            sidecar.write_text(signature(version))
            (root/'staged/windows-x86_64/windows-x86_64.json').unlink()
            with self.assertRaises(AssertionError): assemble.assemble(root/'staged',root/'incomplete',notes)

if __name__ == '__main__': unittest.main()
