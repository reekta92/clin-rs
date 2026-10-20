"""Exercise candidate selection, real adapter failures, and checkout isolation."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/check_upstream.py'
TESTS = {'pinstar': 'pinstar_plugin_e2e', 'graf-rs': 'graf_plugin_e2e',
         'draw-rs': 'draw_plugin_e2e'}


class UpstreamCheckTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.consumer = self.root / 'clin'
        self.env = {**os.environ, 'CARGO_HOME': str(self.root / 'cargo'),
                    'CARGO_TARGET_DIR': str(self.root / 'target')}

    def run_command(self, *args, cwd=None, **kwargs):
        return subprocess.run(args, cwd=cwd or self.consumer, env=self.env,
                              text=True, capture_output=True, **kwargs)

    def git(self, *args, cwd=None):
        return self.run_command('git', *args, cwd=cwd, check=True)

    def commit(self, directory):
        self.git('init', '--initial-branch=main', cwd=directory)
        self.git('config', 'user.name', 'Compatibility Test', cwd=directory)
        self.git('config', 'user.email', 'compat@example.invalid', cwd=directory)
        self.git('add', '.', cwd=directory)
        self.git('commit', '-m', 'Fixture', cwd=directory)

    def fixture(self, package='pinstar', version='1.0.0', source='pub fn value() -> u32 { 42 }\n'):
        upstream = self.root / 'released engine'
        self.candidate = self.root / 'candidate engine 🖊'
        for directory, body, number in [(upstream, 'pub fn value() -> u32 { 7 }\n', '1.0.0'),
                                         (self.candidate, source, version)]:
            (directory / 'src').mkdir(parents=True)
            (directory / 'Cargo.toml').write_text(
                f'[package]\nname = "{package}"\nversion = "{number}"\nedition = "2021"\n')
            (directory / 'src/lib.rs').write_text(body)
        self.commit(upstream)
        (self.consumer / 'src').mkdir(parents=True)
        (self.consumer / 'scripts').mkdir()
        (self.consumer / 'tests').mkdir()
        shutil.copyfile(SCRIPT, self.consumer / 'scripts/check_upstream.py')
        (self.consumer / 'Cargo.toml').write_text(
            '[package]\nname = "consumer"\nversion = "1.0.0"\nedition = "2021"\n'
            f'[dependencies]\nengine = {{ package = "{package}", version = "1", git = "{upstream.as_uri()}" }}\n')
        (self.consumer / 'src/lib.rs').write_text('pub fn value() -> u32 { engine::value() }\n')
        (self.consumer / f'tests/{TESTS[package]}.rs').write_text(
            '#[test]\nfn candidate_contract() { assert_eq!(consumer::value(), 42); }\n')
        self.run_command('cargo', '+stable', 'generate-lockfile', check=True)
        self.commit(self.consumer)
        self.original = {name: (self.consumer / name).read_bytes() for name in ['Cargo.toml', 'Cargo.lock']}
        self.env['CARGO_NET_OFFLINE'] = 'true'

    def check(self):
        result = self.run_command('python3', 'scripts/check_upstream.py', str(self.candidate))
        self.assertEqual({name: (self.consumer / name).read_bytes() for name in self.original}, self.original)
        self.assertEqual(self.git('status', '--porcelain').stdout, '')
        return result

    def test_all_engines_replace_the_git_source_without_changing_the_pin(self):
        for package in TESTS:
            with self.subTest(package=package), tempfile.TemporaryDirectory() as directory:
                self.root = Path(directory)
                self.consumer = self.root / 'clin'
                self.env.update(CARGO_HOME=str(self.root / 'cargo'), CARGO_TARGET_DIR=str(self.root / 'target'))
                self.env.pop('CARGO_NET_OFFLINE', None)
                self.fixture(package)
                result = self.check()
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn('1 passed', result.stdout)

    def test_incompatible_api_fails_without_changing_the_consumer(self):
        self.fixture(source='pub fn renamed() -> u32 { 42 }\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('cannot find function `value`', result.stderr)

    def test_incompatible_version_cannot_silently_test_the_old_git_dependency(self):
        self.fixture(version='2.0.0')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('candidate was not selected', result.stderr)

    def test_unknown_engine_is_rejected(self):
        self.fixture()
        (self.candidate / 'Cargo.toml').write_text('[package]\nname = "unrelated"\nversion = "1.0.0"\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Expected a pinstar, graf-rs, or draw-rs package', result.stderr)


if __name__ == '__main__':
    unittest.main()
