from pathlib import Path
import base64
import shutil
import zipfile

parts = sorted(Path("source_parts").glob("part*.txt"))
if not parts:
    raise SystemExit("source_parts not found")

payload = "".join(p.read_text(encoding="utf-8").strip() for p in parts)
archive = Path("_senryu_source.zip")
archive.write_bytes(base64.b64decode(payload))

tmp = Path("_senryu_source")
if tmp.exists():
    shutil.rmtree(tmp)

with zipfile.ZipFile(archive) as z:
    z.extractall(tmp)

src = tmp / "senryu_bot_rust"
if not src.is_dir():
    raise SystemExit("senryu_bot_rust directory missing from archive")

for item in src.iterdir():
    # Keep the current publishing workflow and source archive stored in GitHub.
    if item.name == ".github":
        continue
    dest = Path(item.name)
    if dest.exists():
        if dest.is_dir():
            shutil.rmtree(dest)
        else:
            dest.unlink()
    if item.is_dir():
        shutil.copytree(item, dest)
    else:
        shutil.copy2(item, dest)

print("Restored complete senryu-bot source archive.")
