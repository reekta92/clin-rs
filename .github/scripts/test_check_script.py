"""Check that the local runner cannot report success after a required gate fails."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/check.sh'


class CheckScriptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'scripts').mkdir()
        shutil.copyfile(SCRIPT, self.root / 'scripts/check.sh')
        self.manifest('1.88')
        tools = self.root / 'tools'
        tools.mkdir()
        cargo = tools / 'cargo'
        cargo.write_text('''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
with open(os.environ['COMMAND_LOG'], 'a') as log:
    print(json.dumps({'args': sys.argv[1:], 'cwd': str(Path.cwd())}), file=log)
if os.environ.get('FAIL_GATE') in sys.argv[1:]:
    sys.exit(7)
''')
        cargo.chmod(0o755)
        self.log = self.root / 'commands'
        self.env = {**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH'],
                    'COMMAND_LOG': str(self.log)}

    def manifest(self, version):
        (self.root / 'Cargo.toml').write_text(f'[package]\nname = "fixture"\nrust-version = "{version}"\n')

    def run_check(self, *args, **env):
        return subprocess.run(['bash', str(self.root / 'scripts/check.sh'), *args],
                              cwd=self.root.parent, env={**self.env, **env},
                              text=True, capture_output=True)

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def test_default_gates_match_ci_and_use_declared_msrv(self):
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call['args'] for call in self.calls()], [
            ['+stable', 'fmt', '--all', '--', '--check'],
            ['+stable', 'clippy', '--locked', '--all-targets', '--', '-D', 'warnings'],
            ['+stable', 'test', '--locked'],
            ['deny', '--locked', '--all-features', 'check'],
            ['+1.88.0', 'check', '--locked', '--all-targets'],
        ])
        self.assertTrue(all(call['cwd'] == str(self.root) for call in self.calls()))

    def test_requested_subset_preserves_order(self):
        result = self.run_check('test', 'clippy')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call['args'][1] for call in self.calls()], ['test', 'clippy'])
        self.assertIn('Passed requested gates: test clippy', result.stdout)

    def test_unknown_gate_fails_before_any_command_runs(self):
        result = self.run_check('fmt', 'typo')
        self.assertEqual(result.returncode, 2)
        self.assertEqual(self.calls(), [])

    def test_failed_gate_stops_without_reporting_success(self):
        result = self.run_check('clippy', 'test', FAIL_GATE='clippy')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.calls()), 1)
        self.assertNotIn('Passed requested gates', result.stdout)

    def test_missing_checker_and_toolchain_cannot_be_skipped(self):
        for gate, failure in [('deny', 'deny'), ('msrv', '+1.88.0')]:
            with self.subTest(gate=gate):
                result = self.run_check(gate, FAIL_GATE=failure)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn('Passed requested gates', result.stdout)

    def test_invalid_msrv_fails_before_cargo_runs(self):
        self.manifest('stable')
        result = self.run_check('msrv')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('explicit rust-version is required', result.stderr)
        self.assertEqual(self.calls(), [])


if __name__ == '__main__':
    unittest.main()
