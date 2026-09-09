from pathlib import Path

path = Path('src/windows/simple_ui.rs')
text = path.read_text(encoding='utf-8')
old = '                    protected: false,\n                    gpu: None,'
new = '                protected: false,\n                gpu: None,'
count = text.count(old)
if count != 2:
    raise RuntimeError(f'expected two nested Choice constructors, found {count}')
path.write_text(text.replace(old, new), encoding='utf-8', newline='\n')
print('normalized Choice constructor indentation for patch application')
