"""Temporary, exact-match source corrections; no user processes are touched.

This helper is restricted to the validation branch and will be removed before
final integration. Refuse source drift instead of applying fuzzy edits.
"""
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise RuntimeError(f"Expected one reviewed match in {path}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")


replace_once("src/windows/process.rs", "HashSet, ffi::c_void,", "HashSet,")
# Documented process SYNCHRONIZE access right; this binding version does not
# export the unqualified name used in the original source.
replace_once("src/windows/process.rs", "rights | PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE", "rights | PROCESS_QUERY_LIMITED_INFORMATION | 0x0010_0000")
replace_once("src/windows/ui.rs", "let height = -((15 * dpi) / 96) as i32;", "let height = -(((15 * dpi) / 96) as i32);")
print("Applied the two reviewed Win32 compile corrections.")
