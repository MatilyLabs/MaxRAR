import importlib.util
import pathlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('stage_update', pathlib.Path(__file__).resolve().parent.parent / 'scripts/stage-update.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class StagingTests(unittest.TestCase):
    def test_signed_assets_get_distinct_platform_names_and_encoded_urls(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            source = root / 'source'
            source.mkdir()
            asset = source / 'MaxRAR app.app.tar.gz'
            asset.write_bytes(b'fixture')
            pathlib.Path(str(asset) + '.sig').write_text('public test signature')
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

if __name__ == '__main__': unittest.main()
