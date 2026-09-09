"""Small reviewed corrections against the unformatted transport source."""
from pathlib import Path
p = Path('src/windows/simple_ui.rs')
s = p.read_text(encoding='utf-8')
needle = 'const TITLE: u16 = 1;'
assert s.count(needle) == 1
s = s.replace(needle, '// Winuser.h static-control centering style; absent from this binding namespace.\nconst SS_CENTER: i32 = 0x0001;\n' + needle)
s = s.replace('Review recovery warning', 'Recovery options')
p.write_text(s, encoding='utf-8', newline='\n')
