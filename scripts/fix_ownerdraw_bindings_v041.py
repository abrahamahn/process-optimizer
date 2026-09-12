from pathlib import Path

path = Path('src/windows/simple_ui.rs')
text = path.read_text(encoding='utf-8')
anchor = 'const LBS_OWNERDRAWFIXED_STYLE: u32 = 0x0010;\n'
insert = '''const LBS_OWNERDRAWFIXED_STYLE: u32 = 0x0010;
const ODS_SELECTED_STYLE: u32 = 0x0001;
const ODS_FOCUS_STYLE: u32 = 0x0010;

#[repr(C)]
struct DrawItem {
    ctl_type: u32,
    ctl_id: u32,
    item_id: u32,
    item_action: u32,
    item_state: u32,
    hwnd_item: HWND,
    h_dc: HDC,
    rc_item: RECT,
    item_data: usize,
}
'''
if anchor not in text:
    raise RuntimeError('owner-draw constant anchor missing')
text = text.replace(anchor, insert, 1)
text = text.replace('DRAWITEMSTRUCT', 'DrawItem')
text = text.replace('ODS_SELECTED as u32', 'ODS_SELECTED_STYLE')
text = text.replace('ODS_FOCUS as u32', 'ODS_FOCUS_STYLE')
for old, new in [
    ('.CtlID', '.ctl_id'),
    ('.itemID', '.item_id'),
    ('.itemState', '.item_state'),
    ('.hwndItem', '.hwnd_item'),
    ('.hDC', '.h_dc'),
    ('.rcItem', '.rc_item'),
]:
    text = text.replace(old, new)
path.write_text(text, encoding='utf-8', newline='\n')
print('owner-draw ABI made local, snake-case, and windows-sys-version independent')
