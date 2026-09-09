from pathlib import Path

path = Path('scripts/apply_settings_controls_v041.py')
text = path.read_text(encoding='utf-8')
old = """text, count = pattern.subn(replacement, text, count=1)\nif count != 1:\n    raise RuntimeError('list rendering block not found')\n"""
new = """list_start = text.find('        for c in &self.choices {\\n            let status = self')\nlist_anchor = '        unsafe {\\n            SendMessageW(self.h(APPS), LB_SETHORIZONTALEXTENT, 1100, 0);'\nlist_end = text.find(list_anchor, list_start)\nif list_start < 0 or list_end < 0:\n    raise RuntimeError('list rendering anchors not found')\ntext = text[:list_start] + replacement + text[list_end + len(list_anchor):]\n"""
if old not in text:
    raise RuntimeError('list regex driver block missing')
path.write_text(text.replace(old, new, 1), encoding='utf-8', newline='\n')
print('made list renderer patch anchor-based')
