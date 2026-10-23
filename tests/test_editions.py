"""Release contracts: Light is empty, Full includes real tools and safe alternatives."""
import importlib.util,json,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('package_editions',ROOT/'scripts/package-editions.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class EditionContracts(unittest.TestCase):
    def test_light_does_not_pull_tools(self):
        self.assertEqual(module.info('light','deb')['bundled_tools'],[])
        self.assertEqual(module.info('light','deb')['distribution_tools'],[])
        self.assertEqual(module.info('light','rpm')['distribution_tools'],[])
    def test_full_contains_requested_tools(self):
        self.assertEqual(set(module.info('full','deb')['bundled_tools']),{'ghidra','cutter','radare2'})
        self.assertIn('gdb',module.info('full','deb')['distribution_tools'])
        self.assertIn('yara',module.info('full','rpm')['distribution_tools'])
    def test_official_artifacts_have_checksums_and_java(self):
        pins=json.loads((ROOT/'packaging/editions/full-sources.json').read_text())['tools']
        self.assertEqual({p['id'] for p in pins},{'ghidra','cutter','radare2','jdk'})
        for p in pins:
            self.assertTrue(p['url'].startswith('https://github.com/'))
            self.assertEqual(len(bytes.fromhex(p['sha256'])),32)
            self.assertEqual(Path(p['filename']).name,p['filename'])
    def test_native_profiles_do_not_embed_nested_installers(self):
        source=(ROOT/'scripts/package-editions.py').read_text()
        self.assertNotIn('postinst',source.split('import argparse')[1])
        self.assertIn('Conflicts: retoolkit-linux-{other}',source)
        self.assertIn('Replaces: retoolkit-linux, retoolkit-linux-{other}',source)
if __name__=='__main__':unittest.main()
