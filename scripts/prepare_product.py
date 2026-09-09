"""One-off reviewed source transport; never part of the delivered application."""
from pathlib import Path
import base64
import hashlib
import json
import lzma

encoded = ''.join(Path(f'scripts/product-part-{n}.txt').read_text(encoding='utf-8') for n in range(5))
payload = base64.b64decode(encoded, validate=True)
assert hashlib.sha256(payload).hexdigest() == 'c81eae381cd4aa8710750e633461ca92ae0213f3e8e028da43aef29b6b79eb3c', 'Source transport checksum mismatch'
records = json.loads(lzma.decompress(payload))
prepared = []
for record in records:
    path = Path(record['path'])
    assert not path.is_absolute() and '..' not in path.parts, 'Invalid source path'
    if record['sha256'] is None:
        assert not path.exists(), f'New source already exists: {path}'
        original = ''
    else:
        original = path.read_text(encoding='utf-8')
        assert hashlib.sha256(original.encode('utf-8')).hexdigest() == record['sha256'], f'Source drift: {path}'
    lines = original.splitlines(keepends=True)
    for start, end, replacement in reversed(record['edits']):
        assert 0 <= start <= end <= len(lines)
        lines[start:end] = [replacement]
    result = ''.join(lines)
    assert hashlib.sha256(result.encode('utf-8')).hexdigest() == record['result_sha256'], f'Patch mismatch: {path}'
    prepared.append((path, result))
for path, result in prepared:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(result, encoding='utf-8', newline='\n')

# Validate the discriminant before reading its native union member.
p = Path('src/windows/reopen.rs')
s = p.read_text(encoding='utf-8')
old = '''            let level = info.Data.WTSInfoExLevel1;
            info.Level == 1 && level.SessionId == session_id && level.SessionState == WTSActive && level.SessionFlags == WTS_SESSIONSTATE_UNLOCK as i32'''
new = '''            if info.Level != 1 { false } else {
                let level = info.Data.WTSInfoExLevel1;
                level.SessionId == session_id && level.SessionState == WTSActive && level.SessionFlags == WTS_SESSIONSTATE_UNLOCK as i32
            }'''
assert s.count(old) == 1, 'Unexpected native union access'
p.write_text(s.replace(old, new), encoding='utf-8', newline='\n')
print(f'Applied {len(prepared)} checksum-verified source changes.')
