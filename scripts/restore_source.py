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


# Compatibility fixes found by CI against current sqlx/rand releases.
db_file = Path("src/db.rs")
db_text = db_file.read_text(encoding="utf-8")
db_text = db_text.replace(
    "sqlx::query(sql).execute(pool).await?;",
    "sqlx::query(*sql).execute(pool).await?;",
)
db_text = db_text.replace(
    """        let mut rng = rand::rng();
        let mut out = Vec::with_capacity(3);
        for _ in 0..3 {
            let offset = rng.random_range(0..count);""",
    """        let offsets: Vec<i64> = {
            let mut rng = rand::rng();
            (0..3).map(|_| rng.random_range(0..count)).collect()
        };
        let mut out = Vec::with_capacity(3);
        for offset in offsets {""",
)
db_file.write_text(db_text, encoding="utf-8")

commands_file = Path("src/commands.rs")
commands_text = commands_file.read_text(encoding="utf-8")
commands_text = commands_text.replace(
    "use std::{collections::HashMap, sync::Arc};",
    "use std::sync::Arc;",
)
commands_file.write_text(commands_text, encoding="utf-8")
print("Applied CI compatibility fixes.")


# Keep package description intentionally minimal.
Path("README.md").write_text(
    """```bash
pip install senryu-bot
```

```python
from senryu_bot import run

run("DISCORD_BOT_TOKEN")
```
""",
    encoding="utf-8",
)
