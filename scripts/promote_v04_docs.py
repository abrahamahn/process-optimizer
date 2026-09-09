"""Promotion-only documentation sync for the verified v0.4 implementation.
This file stays on the feature branch and is not copied to main.
"""
from pathlib import Path
import os
import re


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8", newline="\n")

# README
path = Path("README.md")
text = path.read_text(encoding="utf-8")
if "## New in 0.4.0 — two cleanup choices" not in text:
    section = '''## New in 0.4.0 — two cleanup choices

Settings now separates **Game Mode only** from **Permanent cleanup**.

- **Game Mode only** is temporary: approved apps may be normally closed or have supported CPU/EcoQoS/memory priorities reduced when you press **Turn ON**. **Turn OFF** restores settings changed by the optimizer; it does not resurrect closed apps or unsaved work.
- **Permanent cleanup** is deliberately narrow and reversible in 0.4.0: it can disable an exact matching **current-user Windows `Run` startup entry** after backing up the original value. It does **not** uninstall applications, disable Windows services/drivers/scheduled tasks, change security features, or force-close a running process. Unsupported startup sources are left untouched.
- **Copy / paste process list** opens a read-only text view. Drag-select text, or press **Ctrl+A → Ctrl+C**, and paste it directly into ChatGPT. The copied report omits executable paths, command lines, document/window content, SIDs, and account identifiers.

The Settings list also adds plain-language hints such as **Web browser**, **Overlay / monitoring**, **Developer tool**, **Apple sync / discovery**, **Audio hardware software**, and **ASUS utility**. These hints are guidance, not automatic permission: unknown and unapproved apps remain untouched.

'''
    marker = "## New in 0.3.0 — just Game Mode ON / OFF"
    if marker not in text:
        raise RuntimeError("README v0.3 marker missing")
    text = text.replace(marker, section + marker, 1)
if "| Permanent cleanup |" not in text:
    marker = "| Recovery | Durable SQLite intent before mutation;"
    idx = text.find(marker)
    if idx < 0:
        raise RuntimeError("README implementation table recovery row missing")
    row_end = text.find("\n", idx)
    text = text[:idx] + "| Permanent cleanup | Reversible exact current-user `Run` startup disable/restore with backup-before-delete and conflict-safe restore; no service/task/driver fallback |\n" + text[idx:]
old = "2. For first-use permissions, open **Settings**. Choose optional background apps, confirm a normal-close or Reduce load rule, then select **Done**. Do not approve games or apps needed for voice, accessibility, controllers or device management."
new = "2. Open **Settings**. For temporary gaming cleanup, choose **Close during Game Mode** or **Reduce during Game Mode**. For an unused app that actually has a supported current-user Windows startup entry, choose **Disable Windows startup**. Use **Restore Windows startup** to undo that persistent change."
if old in text:
    text = text.replace(old, new, 1)
write("README.md", text)

# Product spec
path = Path("specs/01-product.md")
text = path.read_text(encoding="utf-8")
text, count = re.subn(
    r"^P-03: .*?$",
    "P-03: Overclocking, undervolting, fan/firmware control, irreversible debloating/service deletion, security disabling, kernel patching, driver reset, game injection, anti-cheat bypass and a stripped boot shell are excluded. Reversible, explicitly approved current-user startup cleanup is in scope only when the original startup record is durably backed up before mutation and conflict-safe restoration is available.",
    text,
    count=1,
    flags=re.MULTILINE,
)
if count != 1:
    raise RuntimeError("specs/01 P-03 missing")
if "M-07:" not in text:
    anchor = "M-06:"
    idx = text.find(anchor)
    if idx < 0:
        raise RuntimeError("specs/01 M-06 missing")
    block = '''M-07: Settings presents two plain-language choices. **Game Mode only** applies temporary close/reduce actions only at a user-initiated On. **Permanent cleanup** persists across reboots but version 0.4 supports only exact current-user `Run` startup values whose executable token matches the reviewed application. The original value is stored before deletion. Restoration refuses to overwrite a conflicting value. This feature never implies uninstalling the app, disabling services/drivers/tasks, or closing a running process. Unsupported sources have no fallback.\n\nM-08: Settings exposes a copyable process report. The user can drag-select text or use Ctrl+A/Ctrl+C and paste it into support/chat. The report may include PID, application display name, bounded GPU/memory observation and protection status, but excludes executable paths, command lines, document/window content, SIDs and account identifiers.\n\n'''
    text = text[:idx] + block + text[idx:]
write("specs/01-product.md", text)

# GPU/process policy
path = Path("specs/02-gpu-and-process-policy.md")
text = path.read_text(encoding="utf-8")
if "## Persistent startup cleanup" not in text:
    anchor = "## Unsupported features and future adapters"
    idx = text.find(anchor)
    if idx < 0:
        raise RuntimeError("specs/02 unsupported-features marker missing")
    block = '''## Persistent startup cleanup

Persistent cleanup is **not** a gaming-session process mutation. Version 0.4 may enumerate and edit only `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run` string values. An entry is eligible only when its first executable token can be conservatively resolved to the reviewed canonical executable. Environment-variable or shell-command ambiguity is unsupported.

Before deletion, store the exact value name, command string and value kind. Re-read immediately before deleting it; observable drift cancels the operation. Restoration never overwrites another value using the same name. No process is terminated by this action.

Do not fall back to disabling machine-wide startup, services, drivers, scheduled tasks, security software, device/audio/Bluetooth components, shell components or vendor utilities that cannot be attributed through the supported source. Future service/task cleanup requires its own owner contract, privilege design, dependency checks and native tests.

'''
    text = text[:idx] + block + text[idx:]
write("specs/02-gpu-and-process-policy.md", text)

# Validation spec
path = Path("specs/04-validation-and-delivery.md")
text = path.read_text(encoding="utf-8")
anchor = "| Empty current matches in manual mode | Honest no-change state, never fallback to wildcard cleanup |"
if anchor not in text:
    raise RuntimeError("specs/04 manual-empty acceptance row missing")
if "| Copyable process report |" not in text:
    extra = anchor + '''
| Copyable process report | Drag/Ctrl+A/Ctrl+C-ready plain text; no executable paths, command lines, documents, SIDs or account identifiers |
| Permanent cleanup unsupported source | Report unsupported and make no change; no service/task/kill fallback |
| Permanent cleanup supported `Run` entry | Backup the exact original value before deletion and verify it is absent afterwards |
| Permanent cleanup entry changes after review | Abort deletion; preserve the current startup value |
| Restore startup name conflict | Preserve the conflicting value; never overwrite it |'''
    text = text.replace(anchor, extra, 1)
if "### Version 0.4.0 two-mode cleanup promotion" not in text:
    marker = "### Prior evidence"
    idx = text.find(marker)
    if idx < 0:
        raise RuntimeError("specs/04 prior-evidence marker missing")
    run_id = os.environ.get("GITHUB_RUN_ID", "promotion-run")
    source_sha = os.environ.get("V04_SOURCE_SHA", "verified-feature-source")
    evidence = f'''### Version 0.4.0 two-mode cleanup promotion

Promotion gate `{run_id}` validated feature source `{source_sha}` on GitHub-hosted Windows before updating `main`. The gate runs locked Rust tests, the separately opted-in disposable force fixture, standard-user worker lifecycle fixtures, warning-free Clippy, a release build, native-window smoke and a read-only GPU probe. The Windows test suite also includes a unique test-owned HKCU `Run` value that is created, enumerated, disabled, restored and removed without touching arbitrary user startup entries.

This establishes the tested software behavior on the hosted Windows environment only. It does **not** establish physical gaming performance, third-party service cleanup, driver/HAGS compatibility or FPS improvement on the user's gaming laptop. Permanent cleanup remains limited to the supported current-user `Run` source documented above.

'''
    text = text[:idx] + evidence + text[idx:]
write("specs/04-validation-and-delivery.md", text)

print("v0.4 documentation synchronized for promotion")
