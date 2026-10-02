"""Certified references for the Dualray Fast prototype, from gma-accuracy's oracle.

Run with gma-accuracy's virtual environment (python-flint):

  $GMA_ACCURACY/.venv/bin/python scripts/dualray-fast-oracle.py reference <dir> <gamut> <lane>
  $GMA_ACCURACY/.venv/bin/python scripts/dualray-fast-oracle.py standard <dir>

`reference` reads <dir>/<gamut>-<lane>.in (binary64 rows L, C, h, written by
`cargo run --example dualray-fast -- export <dir>`) and writes
<dir>/<gamut>-<lane>.ref. `standard` copies gma-accuracy's certified standard
tier references (repo-p3-b64-v1) for the primary contract's inputs (finite,
L in [0,1], C >= 0, hue in [0,360]) to <dir>/standard-<lane>.ref. Reference rows
are little-endian binary64: L, C, h, membership (0 inside, 1 outside,
2 endpoint convention, 3 unresolved or not applicable) and the midpoints of
the certified encoded-output enclosures (width at most 2^-60).

Display P3 uses gma-accuracy's frozen repo-p3-b64-v1 profile; the script
checks that gma-benchmark's binary64 matrices are bit-identical to it. sRGB and
Rec.2020 are ad hoc profiles built the same way from gma-benchmark's binary64
matrices and transfer constants. They use gma-accuracy's oracle code but are
not frozen gma-accuracy profiles. Rec.2020's pure 2.4 gamma is evaluated as a
monotone endpoint power, since the piecewise sRGB form has no pure-power case.
"""
import json
import multiprocessing
import os
import sqlite3
import struct
import sys
import zlib
from fractions import Fraction
from pathlib import Path

GMA = Path(os.environ.get('GMA_ACCURACY', '/home/lloydk/work/gma-accuracy'))
sys.path.insert(0, str(GMA))
from flint import arb  # noqa: E402
from reference import oracle, profiles  # noqa: E402
from reference.balls import ball, bounds, ieee_value  # noqa: E402

FROZEN = 'repo-p3-b64-v1'
CUSTOM = {}

def pure_gamma_encode(original):
    def encode_channel(x, p):
        if not p.get('pure_gamma'):
            return original(x, p)
        # Clamp the exact endpoints: a clamped ball's lower bound can be a
        # tiny negative after Arb's midpoint-radius union.
        a, b = (min(max(v, Fraction(0)), Fraction(1)) for v in bounds(x))
        e = ball(p['encode_exponent'])
        image = lambda v: arb(0) if v == 0 else ball(v) ** e
        return image(a).union(image(b))
    return encode_channel

def load_profile(directory, gamut):
    """Profile dict for oracle.solve; P3 must match the frozen profile exactly."""
    data = json.loads((Path(directory) / f'{gamut}.profile.json').read_text())
    matrix = lambda rows: [[ieee_value(v) for v in row] for row in rows]
    frozen = profiles.profile(FROZEN)
    mats = {
        'oklab_to_lms_prime': matrix(data['oklab_to_lms_prime']),
        'lms_to_linear_p3': matrix(data['lms_to_linear']),
        'linear_p3_to_lms': matrix(data['linear_to_lms']),
        'lms_prime_to_oklab': frozen['matrices']['lms_prime_to_oklab'],
    }
    if gamut == 'display-p3':
        for key in ['oklab_to_lms_prime', 'lms_to_linear_p3', 'linear_p3_to_lms']:
            assert mats[key] == frozen['matrices'][key], f'{key} differs from {FROZEN}'
        return FROZEN
    assert mats['oklab_to_lms_prime'] == frozen['matrices']['oklab_to_lms_prime']
    profile = dict(frozen, id=f'adhoc-{gamut}-b64', matrices=mats, sha256='adhoc')
    if gamut == 'rec2020':
        profile['pure_gamma'] = True
    elif gamut != 'srgb':
        raise ValueError(gamut)
    CUSTOM[profile['id']] = profile
    return profile['id']

def install(directory, gamut):
    """Per process: patch the oracle's profile lookup and encoder."""
    profiles.encode_channel = pure_gamma_encode(profiles.encode_channel)
    frozen_lookup = profiles.profile
    lookup = lambda name: CUSTOM[name] if name in CUSTOM else frozen_lookup(name)
    oracle.profile = lookup
    return load_profile(directory, gamut)

NAME = None

def init(directory, gamut):
    global NAME
    NAME = install(directory, gamut)

def midpoint(enclosure):
    lo = Fraction(int(enclosure['lo'][0])) * Fraction(2) ** int(enclosure['lo'][1])
    hi = Fraction(int(enclosure['hi'][0])) * Fraction(2) ** int(enclosure['hi'][1])
    return float((lo + hi) / 2)

MEMBERSHIP = {'inside': 0, 'outside': 1, 'endpoint-convention': 2}

def row_of(L, C, h, record):
    if record.get('status') != 'certified':
        return (L, C, h, 3.0, 0.0, 0.0, 0.0)
    m = MEMBERSHIP.get(record['membership'], 3)
    out = [midpoint(x) for x in record['output']] if m < 3 else [0.0] * 3
    return (L, C, h, float(m), *out)

def work(chunk):
    rows = []
    for L, C, h in chunk:
        bits = [struct.pack('>d', v).hex() for v in (L, C, h)]
        rows.append(row_of(L, C, h, oracle.solve(bits, NAME)))
    return rows

def reference(directory, gamut, lane):
    data = (Path(directory) / f'{gamut}-{lane}.in').read_bytes()
    inputs = list(struct.iter_unpack('<3d', data))
    chunks = [inputs[i:i + 500] for i in range(0, len(inputs), 500)]
    name = install(directory, gamut)
    with multiprocessing.Pool(int(os.environ.get('WORKERS', '12')), init, (directory, gamut)) as pool:
        rows = [row for part in pool.imap(work, chunks) for row in part]
    out = Path(directory) / f'{gamut}-{lane}.ref'
    out.write_bytes(b''.join(struct.pack('<7d', *r) for r in rows))
    counts = {}
    for r in rows:
        counts[r[3]] = counts.get(r[3], 0) + 1
    print(f'{out}: {len(rows)} references with {name}; membership counts {counts}')

def standard(directory):
    folder = GMA / 'artifacts/references/standard' / FROZEN
    meta = json.loads((folder / 'manifest.json').read_text())
    assert meta['oracle_source_sha256'] == oracle.source_hash(), 'stale gma-accuracy reference'
    db = sqlite3.connect((folder / 'records.sqlite').as_uri() + '?mode=ro&immutable=1', uri=True)
    for bits, lane in [('64', 'f64'), ('32', 'f32')]:
        rows, excluded = [], 0
        for entry in meta['lanes'][bits]:
            data = db.execute('SELECT data FROM records WHERE key=?', (entry['key'],)).fetchone()
            record = json.loads(zlib.decompress(data[0])) if data else {}
            try:
                L, C, h = [float(ieee_value(v)) for v in record['input_bits']]
            except (KeyError, ValueError, OverflowError):
                excluded += 1
                continue
            if not (0 <= L <= 1 and C >= 0 and 0 <= h <= 360):
                excluded += 1
                continue
            rows.append(row_of(L, C, h, record))
        out = Path(directory) / f'standard-{lane}.ref'
        out.write_bytes(b''.join(struct.pack('<7d', *r) for r in rows))
        print(f'{out}: {len(rows)} certified rows, {excluded} outside the primary contract')

if __name__ == '__main__':
    command, *args = sys.argv[1:]
    {'reference': reference, 'standard': standard}[command](*args)
