"""Temporary reviewed patch transport; never included in the final main tree."""
from pathlib import Path
import base64
import gzip
import hashlib
import subprocess

path = Path('scripts/recovery-audit.patch.gz')
encoded = base64.b64encode(path.read_bytes()).decode('ascii')
# Correct known transcription changes in the transport, never fuzz source code.
for wrong, right in [
    ('CzK50XcFjSIKLL', 'CzK50XcVjSIKLL'),
    ('Urlra2ebaeTXUs', 'Urlra2ebTXUs'),
    ('ES1a2t5eNhthbw', 'ES1a2t5Nthbw'),
]:
    encoded = encoded.replace(wrong, right)
archive = base64.b64decode(encoded)
expected = '24980aca1ecc363c435f5f6be0bace37df63e917c6c89d797aa81cff7b1e7c68'
actual = hashlib.sha256(archive).hexdigest()
if actual != expected:
    raise RuntimeError(f'Patch transport checksum mismatch: {actual}; no source edits allowed')
patch = gzip.decompress(archive)
subprocess.run(['git', 'apply', '--check', '--whitespace=error-all', '-'], input=patch, check=True)
subprocess.run(['git', 'apply', '--whitespace=error-all', '-'], input=patch, check=True)
print('Exact reviewed source patch applied after SHA-256 verification.')
