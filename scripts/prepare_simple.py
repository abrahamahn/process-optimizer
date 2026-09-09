"""Temporary checksum-verified source transport; removed from delivery."""
from pathlib import Path
import base64, hashlib, json, lzma, runpy

text = ''.join(Path(f'scripts/simple-part-{n}.txt').read_text(encoding='utf-8') for n in range(2))
payload = base64.b64decode(text, validate=True)
assert hashlib.sha256(payload).hexdigest() == '6e8cdc78dc7c8c799e951d94c9eeb66dc5502d9eb896d70f78836fb157f15035', 'Source transport checksum mismatch'
records = json.loads(lzma.decompress(payload))
prepared = []
for record in records:
    path = Path(record['path'])
    assert not path.is_absolute() and '..' not in path.parts
    if record['before'] is None:
        assert not path.exists(), f'New source already exists: {path}'
        original = ''
    else:
        original = path.read_text(encoding='utf-8')
        assert hashlib.sha256(original.encode()).hexdigest() == record['before'], f'Source drift: {path}'
    lines = original.splitlines(keepends=True)
    for start, end, replacement in reversed(record['edits']):
        assert 0 <= start <= end <= len(lines)
        lines[start:end] = [replacement]
    result = ''.join(lines)
    assert hashlib.sha256(result.encode()).hexdigest() == record['after'], f'Patch mismatch: {path}'
    prepared.append((path, result))
for path, result in prepared:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(result, encoding='utf-8', newline='\n')
if Path('scripts/simple_followup.py').exists():
    runpy.run_path('scripts/simple_followup.py')
print(f'Applied {len(prepared)} checksum-verified source changes; no user process is involved.')
