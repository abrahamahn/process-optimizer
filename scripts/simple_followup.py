"""Small reviewed corrections against the unformatted transport source."""
from pathlib import Path
import subprocess
p = Path('src/windows/simple_ui.rs')
s = p.read_text(encoding='utf-8')
needle = 'const TITLE: u16 = 1;'
assert s.count(needle) == 1
s = s.replace(needle, '// Winuser.h static-control centering style; absent from this binding namespace.\nconst SS_CENTER: i32 = 0x0001;\n' + needle)
s = s.replace('Review recovery warning', 'Recovery options')
needle = '    if msg==WM_NCCREATE '
assert s.count(needle) == 1
s = s.replace(needle, '''    if msg == WM_CTLCOLORSTATIC {
        let dc = w as HDC;
        SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
        SetBkColor(dc, GetSysColor(COLOR_WINDOW));
        return GetSysColorBrush(COLOR_WINDOW) as LRESULT;
    }
''' + needle)
p.write_text(s, encoding='utf-8', newline='\n')
# The CI token may publish source but cannot change workflows. Keep the
# repository's committed normal workflow untouched; the validation workflow
# itself already exercises both native entry points and all lifecycle cases.
subprocess.run(['git', 'restore', '--worktree', '--', '.github/workflows/ci.yml'], check=True)
