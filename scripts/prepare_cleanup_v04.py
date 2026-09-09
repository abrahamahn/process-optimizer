"""Temporary v0.4 source transport. Removed before main delivery.
Only the reviewed whitelist is extracted; the archive checksum is pinned.
"""
from pathlib import Path
import hashlib
import tarfile

ARCHIVE = Path("scripts/v04-payload.tar.xz")
EXPECTED = "e88c84a06a5551ba16dad29d3e2216b215e051335ed8739e11c7e6b29ac24d30"
ALLOWED = {
    "Cargo.toml",
    "README.md",
    "specs/01-product.md",
    "specs/02-gpu-and-process-policy.md",
    "specs/04-validation-and-delivery.md",
    "src/manual.rs",
    "src/journal.rs",
    "src/windows/mod.rs",
    "src/windows/startup.rs",
    "src/windows/simple_ui.rs",
    "tests/manual_mode.rs",
    "tests/windows_hardening.rs",
}

raw = ARCHIVE.read_bytes()
if hashlib.sha256(raw).hexdigest() != EXPECTED:
    raise RuntimeError("v0.4 source archive checksum mismatch")

with tarfile.open(ARCHIVE, "r:xz") as tf:
    members = tf.getmembers()
    names = {m.name for m in members}
    if names != ALLOWED or any(not m.isfile() for m in members):
        raise RuntimeError(f"Unexpected v0.4 source archive members: {sorted(names ^ ALLOWED)}")
    for member in members:
        target = Path(member.name)
        if target.is_absolute() or ".." in target.parts:
            raise RuntimeError("Unsafe archive path")
        source = tf.extractfile(member)
        if source is None:
            raise RuntimeError(f"Missing archive data: {member.name}")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read())

print("Applied checksum-pinned v0.4 source payload.")
