#!/usr/bin/env python3
"""Build once, then overlap the bounded idle-view check with the full remaining suite."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
IDLE_TEST = 'view_perf_tests::static_views_do_not_redraw_while_idle'


def build_tests(features):
    result = subprocess.run(
        ['cargo', 'test', '--locked', '--no-run', '--message-format=json', *features],
        cwd=ROOT, stdout=subprocess.PIPE, text=True, timeout=1200,
    )
    executables = []
    for line in result.stdout.splitlines():
        artifact = json.loads(line)
        if artifact.get('reason') == 'compiler-message':
            print(artifact['message'].get('rendered', ''), file=sys.stderr, end='')
        if (artifact.get('reason') == 'compiler-artifact'
                and artifact.get('profile', {}).get('test')
                and artifact['target']['kind'] == ['lib']
                and artifact['target']['name'] == 'clin'
                and artifact.get('executable')):
            executables.append(artifact['executable'])
    if result.returncode or len(executables) != 1:
        raise RuntimeError('Cargo must successfully build exactly one Clin library test harness')
    executable = executables[0]
    listed = subprocess.run(
        [executable, '--list', '--exact', IDLE_TEST], cwd=ROOT,
        capture_output=True, text=True, timeout=10, check=True,
    )
    if f'{IDLE_TEST}: test' not in listed.stdout.splitlines():
        raise RuntimeError('Idle-view test is missing; refusing to skip its coverage')
    return executable


def run_parallel(runs):
    children = []
    try:
        for label, command, timeout in runs:
            print(f'Running {label} (timeout {timeout}s)', flush=True)
            child = subprocess.Popen(command, cwd=ROOT, start_new_session=os.name != 'nt')
            children.append((label, child, time.monotonic() + timeout))
        while True:
            finished = True
            for label, child, deadline in children:
                code = child.poll()
                if code is not None:
                    if code:
                        raise RuntimeError(f'{label} failed with exit code {code}')
                else:
                    finished = False
                    if time.monotonic() >= deadline:
                        raise TimeoutError(f'{label} timed out')
            if finished:
                return
            time.sleep(0.05)
    finally:
        for _, child, _ in children:
            if child.poll() is None:
                if os.name == 'nt':
                    subprocess.run(
                        ['taskkill', '/PID', str(child.pid), '/T', '/F'],
                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                        timeout=10, check=False,
                    )
                    if child.poll() is None:
                        child.kill()
                else:
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                child.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--all-features', action='store_true')
    args = parser.parse_args()
    features = ['--all-features'] if args.all_features else []
    executable = build_tests(features)
    run_parallel([
        ('idle-view test', [executable, '--exact', IDLE_TEST, '--nocapture'], 30),
        ('remaining tests and doctests',
         ['cargo', 'test', '--locked', *features, '--', '--skip', IDLE_TEST], 300),
    ])


if __name__ == '__main__':
    try:
        main()
    except (RuntimeError, TimeoutError, subprocess.SubprocessError, OSError) as error:
        print(f'::error::{error}', file=sys.stderr)
        sys.exit(1)
