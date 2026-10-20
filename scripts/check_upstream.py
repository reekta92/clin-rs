#!/usr/bin/env python3
"""Run Clin's adapter contract against a local library without changing its pin."""
import argparse
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
TESTS = {'pinstar': 'pinstar_plugin_e2e', 'graf-rs': 'graf_plugin_e2e',
         'draw-rs': 'draw_plugin_e2e'}


def check(library):
    library = library.resolve(strict=True)
    package = tomllib.loads((library / 'Cargo.toml').read_text()).get('package', {}).get('name')
    if package not in TESTS:
        raise ValueError('Expected a pinstar, graf-rs, or draw-rs package')
    revision = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
    archive = subprocess.check_output(['git', '-C', str(ROOT), 'archive', '--format=tar', revision])
    env = {**os.environ, 'CARGO_TARGET_DIR': os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target'))}
    with tempfile.TemporaryDirectory(prefix='clin-compat-') as directory:
        consumer = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(archive)) as source:
            source.extractall(consumer, filter='data')
        manifest = tomllib.loads((consumer / 'Cargo.toml').read_text())
        dependency = next((value for alias, value in manifest.get('dependencies', {}).items()
                           if isinstance(value, dict) and value.get('package', alias) == package), None)
        if dependency is None or 'git' not in dependency:
            raise ValueError(f'Clin must have a direct Git dependency on {package}')
        override = consumer / 'upstream.toml'
        override.write_text(f'[patch.{json.dumps(dependency["git"])}]\n'
                            f'{json.dumps(package)} = {{ path = {json.dumps(str(library))} }}\n')
        cargo = ['cargo', '+stable', '--config', str(override)]
        print(f'Checking {package} from {library} against Clin {revision}', flush=True)
        subprocess.run([*cargo, 'update', '-p', package], cwd=consumer, env=env, check=True)
        metadata = json.loads(subprocess.check_output(
            [*cargo, 'metadata', '--locked', '--format-version', '1'], cwd=consumer, env=env))
        candidate = next((item for item in metadata['packages']
                          if item['name'] == package
                          and Path(item['manifest_path']).resolve() == library / 'Cargo.toml'), None)
        root = next(node for node in metadata['resolve']['nodes']
                    if node['id'] == metadata['resolve']['root'])
        if candidate is None or candidate['id'] not in root['dependencies']:
            raise ValueError(f'The candidate was not selected for Clin\'s direct {package} dependency; check its version requirement')
        subprocess.run([*cargo, 'test', '--locked', '--test', TESTS[package]],
                       cwd=consumer, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('library', type=Path, help='Path to the candidate library checkout')
    args = parser.parse_args()
    try:
        check(args.library)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
