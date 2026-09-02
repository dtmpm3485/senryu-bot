# senryu-bot

RustでDiscord BOT本体を動かし、Python側はトークンを渡して起動するだけの川柳検出BOTです。

PyPI公開後は:

```bash
pip install senryu-bot
```

ソースから直接入れる場合は:

```bash
pip install .
```

```python
from senryu_bot import run

run("DISCORD_BOT_TOKEN")
```

`run()` はBOTが終了するまでブロックします。Ctrl+C / SIGTERM で安全に終了します。

## 維持する機能

- メッセージから 5-7-5 を自動検出・保存
- `詠め` / `詠むな`
- `/mute` / `/unmute`
- `/rank`
- `/delete`（25件ごとのページ送り・選択・確認）
- `/detect on|off|status|ban|unban|list`
- `/channel`
- `/doctor`
- `/contact`
- `/admin stats|backup|contact-message`
- ユーザーオプトアウト
- チャンネルタイプ別ON/OFF
- 親チャンネルのミュート継承
- 権限不足時の自動ミュート
- SQLite / PostgreSQL
- AES-256-GCM暗号化
- SQLite定期バックアップ
- `/health` `/ready` `/stats` `/metrics`
- サーバー参加時ウェルカム
- サーバー脱退時データ削除
- 管理チャンネルへの参加/脱退通知・定期レポート
- Discord推奨シャード数を使った自動シャーディング

## 利便性

通常は設定ファイル不要です。`run("TOKEN")` だけで `data/senryu.db` を自動作成します。スラッシュコマンド登録、DB初期化、シャーディング、ヘルスサーバーまでRust側で行います。

高度な設定が必要なときだけ `senryu_bot.toml` または `config.toml` を置きます。
トークン引数が最優先です。

環境変数は `SENRYU_BOT_` を推奨し、移行用に `FINDSENRYU_` も読めます。

トークンはコードへの直書きより環境変数などから渡す運用を推奨します。API自体は `run(token)` のままです。

## ビルド

```bash
python -m pip install maturin
maturin develop --release
```

wheel:

```bash
maturin build --release
```

PyPI:

```bash
maturin publish
```

## Discord設定

Developer Portalで **Message Content Intent** を有効化してください。

最低限:
- View Channel
- Send Messages
- Read Message History

推奨:
- Embed Links
- Use External Emojis

## Attribution

FindSenryu4Discord (u16-io, MIT) の利用者向け挙動を参考にした独立Rust再実装です。元プロジェクトと検出ロジック参照元の表記は `THIRD_PARTY_NOTICES.md` に収録しています。
