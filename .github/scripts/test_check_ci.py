"""Keep CI coverage, failure propagation, and process deadlines intact."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/test_ci.py'
SPEC = importlib.util.spec_from_file_location('test_ci', SCRIPT)
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class CiRunnerTests(unittest.TestCase):
    def test_features_and_doctests_reach_both_test_processes(self):
        for features in [[], ['--all-features']]:
            with self.subTest(features=features):
                artifact = {'reason': 'compiler-artifact', 'profile': {'test': True},
                            'target': {'kind': ['lib'], 'name': 'clin'},
                            'executable': '/built/clin-tests'}
                responses = [subprocess.CompletedProcess([], 0, json.dumps(artifact)),
                             subprocess.CompletedProcess([], 0, runner.IDLE_TEST + ': test\n')]
                with patch.object(runner.subprocess, 'run', side_effect=responses) as build:
                    with patch.object(runner, 'run_parallel') as parallel:
                        with patch.object(sys, 'argv', [str(SCRIPT), *features]):
                            runner.main()
                command = build.call_args_list[0].args[0]
                self.assertIn('--locked', command)
                self.assertIn('--no-run', command)
                self.assertEqual('--all-features' in command, bool(features))
                idle, remaining = parallel.call_args.args[0]
                self.assertEqual(idle[1], ['/built/clin-tests', '--exact', runner.IDLE_TEST,
                                           '--nocapture'])
                self.assertEqual(remaining[1], ['cargo', 'test', '--locked', *features,
                                                '--', '--skip', runner.IDLE_TEST])
                self.assertNotIn('--all-targets', remaining[1])

    def test_missing_or_failed_harness_cannot_skip_coverage(self):
        artifact = {'reason': 'compiler-artifact', 'profile': {'test': True},
                    'target': {'kind': ['lib'], 'name': 'clin'},
                    'executable': '/built/clin-tests'}
        cases = [([subprocess.CompletedProcess([], 7, '')], 'successfully build'),
                 ([subprocess.CompletedProcess([], 0, '')], 'exactly one'),
                 ([subprocess.CompletedProcess([], 0, json.dumps(artifact)),
                   subprocess.CompletedProcess([], 0, '0 tests\n')], 'missing')]
        for responses, message in cases:
            with self.subTest(message=message):
                with patch.object(runner.subprocess, 'run', side_effect=responses):
                    with self.assertRaisesRegex(RuntimeError, message):
                        runner.build_tests([])

    def run_processes(self, runs):
        children = []
        spawn = subprocess.Popen

        def record(*args, **kwargs):
            child = spawn(*args, **kwargs)
            children.append(child)
            return child

        with patch.object(runner.subprocess, 'Popen', side_effect=record):
            try:
                runner.run_parallel(runs)
            finally:
                self.assertTrue(all(child.poll() is not None for child in children))

    def test_waits_for_both_successful_processes(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = [Path(directory) / name for name in ['idle', 'remaining']]
            runs = [(str(path), [sys.executable, '-c',
                     'from pathlib import Path; import sys; Path(sys.argv[1]).write_text("done")',
                     str(path)], 5) for path in paths]
            self.run_processes(runs)
            self.assertTrue(all(path.read_text() == 'done' for path in paths))

    def test_either_failure_stops_and_reaps_the_other_process(self):
        for failing in [0, 1]:
            with self.subTest(failing=failing):
                commands = [[sys.executable, '-c', 'import time; time.sleep(60)'] for _ in range(2)]
                commands[failing] = [sys.executable, '-c', 'raise SystemExit(7)']
                with self.assertRaisesRegex(RuntimeError, 'exit code 7'):
                    self.run_processes([(str(i), command, 5)
                                        for i, command in enumerate(commands)])

    def test_deadline_stops_and_reaps_a_hung_process(self):
        with self.assertRaisesRegex(TimeoutError, 'idle timed out'):
            self.run_processes([
                ('idle', [sys.executable, '-c', 'import time; time.sleep(60)'], 0.05),
                ('remaining', [sys.executable, '-c', 'pass'], 5),
            ])


if __name__ == '__main__':
    unittest.main()
