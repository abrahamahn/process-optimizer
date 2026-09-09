"""Two reviewed native API corrections after the behavioral source transformation."""
from pathlib import Path
import re

p = Path('src/windows/storage.rs')
s = p.read_text(encoding='utf-8')
s, count = re.subn(r'null\(\),(\s*)null\(\),(\s*)acl', r'null_mut(),\1null_mut(),\2acl', s)
if count != 1:
    raise RuntimeError('Expected exactly one owner/group security pointer correction')
p.write_text(s, encoding='utf-8', newline='\n')

p = Path('src/windows/ui.rs')
s = p.read_text(encoding='utf-8')
old = '        unsafe { EnableWindow(self.h(OPT_LAUNCH), 0); }\n'
if s.count(old) != 1:
    raise RuntimeError('Expected exactly one unavailable-launch EnableWindow call')
s = s.replace(old, '', 1)
old = 'WS_TABSTOP | BS_AUTOCHECKBOX as u32'
if s.count(old) != 1:
    raise RuntimeError('Expected exactly one option checkbox construction')
s = s.replace(old, 'WS_TABSTOP | BS_AUTOCHECKBOX as u32 | if id == OPT_LAUNCH { WS_DISABLED } else { 0 }', 1)
p.write_text(s, encoding='utf-8', newline='\n')
print('Corrected Win32 security pointer types and disabled unavailable launch at control creation.')
