"""Self-contained native fixtures, never installed or used as production tools."""
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / 'target' / 'test-work'
EXE = ROOT / 'clink-posix-native.exe'
FIXTURE = WORK / 'fixture.exe'
MANIFEST = WORK / 'tools.tsv'


def prepare_fixture():
    if os.name != 'nt':
        raise SystemExit('This prototype and its tests require Windows.')
    WORK.mkdir(parents=True, exist_ok=True)
    subprocess.run(['rustc', '--edition=2024', '-O', str(ROOT / 'tests' / 'fixture.rs'),
                    '-o', str(FIXTURE)], check=True)
    MANIFEST.write_text(''.join(f'{name}\t{FIXTURE.as_posix()}\t{name}\n'
                               for name in ('cat', 'sort', 'printf', 'diff')), encoding='utf8')


def native_args():
    return [str(EXE), '--tools', str(MANIFEST)]
