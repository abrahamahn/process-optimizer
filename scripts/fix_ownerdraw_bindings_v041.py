from pathlib import Path

path = Path('src/windows/simple_ui.rs')
text = path.read_text(encoding='utf-8')
anchor = 'const LBS_OWNERDRAWFIXED_STYLE: u32 = 0x0010;\n'
insert = '''const LBS_OWNERDRAWFIXED_STYLE: u32 = 0x0010;
const ODS_SELECTED_STYLE: u32 = 0x0001;
const ODS_FOCUS_STYLE: u32 = 0x0010;

#[repr(C)]
struct DrawItem {
    CtlType: u32,
    CtlID: u32,
    itemID: u32,
    itemAction: u32,
    itemState: u32,
    hwndItem: HWND,
    hDC: HDC,
    rcItem: RECT,
    itemData: usize,
}
'''
if anchor not in text:
    raise RuntimeError('owner-draw constant anchor missing')
text = text.replace(anchor, insert, 1)
text = text.replace('DRAWITEMSTRUCT', 'DrawItem')
text = text.replace('ODS_SELECTED as u32', 'ODS_SELECTED_STYLE')
text = text.replace('ODS_FOCUS as u32', 'ODS_FOCUS_STYLE')
path.write_text(text, encoding='utf-8', newline='\n')
print('owner-draw ABI made local and windows-sys-version independent')
