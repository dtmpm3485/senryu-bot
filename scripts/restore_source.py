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




# Keep the package README in sync with GitHub.
Path("README.md").write_text(
    "# senryu-bot\n\n## インストール\n\n```bash\npip install senryu-bot\n```\n\n## 使い方\n\n```python\nfrom senryu_bot import run\n\nrun(\"DISCORD_BOT_TOKEN\")\n```\n\n## コマンド一覧\n\n- `/mute` - このチャンネルで川柳検出を停止\n- `/unmute` - このチャンネルで川柳検出を再開\n- `/rank` - サーバー内の川柳ランキングを表示\n- `/delete` - 自分の川柳を選んで削除\n- `/detect on` - 自分の川柳検出を有効化\n- `/detect off` - 自分の川柳検出を無効化\n- `/detect status` - 川柳検出の状態を確認\n- `/detect ban` - 管理者がユーザーの検出を無効化\n- `/detect unban` - 管理者がユーザーの検出を再有効化\n- `/detect list` - 検出無効ユーザーの一覧を表示\n- `/channel` - チャンネルタイプごとの検出設定\n- `/doctor` - Botの動作状況を確認\n- `/contact` - Bot管理者へ問い合わせ\n- `/admin stats` - 管理者向け統計\n- `/admin backup` - 管理者向けバックアップ\n- `/admin contact-message` - 問い合わせへの返信\n\nメッセージで `詠め` と送ると保存済みの川柳から一句作り、`詠むな` と送ると直前の川柳を表示します。\n",
    encoding="utf-8",
)
