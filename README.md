# senryu-bot — Discordの会話から川柳を自動検出するRust製Bot

[![PyPI](https://img.shields.io/pypi/v/senryu-bot)](https://pypi.org/project/senryu-bot/)
[![Python](https://img.shields.io/pypi/pyversions/senryu-bot)](https://pypi.org/project/senryu-bot/)
[![License](https://img.shields.io/github/license/dtmpm3485/senryu-bot)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/dtmpm3485/senryu-bot?style=social)](https://github.com/dtmpm3485/senryu-bot/stargazers)

**senryu-bot** は、Discordの普段の会話から川柳っぽい発言を自動検出する **Rust製Discord Botライブラリ** です。

サーバーで普通に会話しているだけで、偶然生まれた一句をBotが拾います。Pythonから1関数で起動でき、ランキング・検出ON/OFF・チャンネル設定・管理者機能も利用できます。

> 💬 何気ない会話
>
> 🤖 **川柳を検出しました**
> 「ここに偶然生まれた一句」

## ✨ 特徴

- 🎋 **Discordの通常会話から川柳を自動検出**
- 🦀 **Rust製** — 高速なBot本体と判定処理
- 🐍 **Pythonから簡単起動** — `run(token)` だけ
- 🏆 **サーバー内ランキング**
- 🔕 **ユーザー・チャンネル単位で検出ON/OFF**
- 🗑️ **自分の保存済み川柳を削除可能**
- 🩺 **Bot状態確認コマンド**
- 🛠️ **管理者向け統計・バックアップ・問い合わせ機能**

## 🚀 すぐに使う

### 1. インストール

```bash
pip install -U senryu-bot
```

### 2. Botを起動

```python
from senryu_bot import run

run("DISCORD_BOT_TOKEN")
```

Discord Botトークンを入れてPythonを実行すれば起動できます。

## 📖 コマンド一覧

| コマンド | 内容 |
|---|---|
| `/mute` | このチャンネルで川柳検出を停止 |
| `/unmute` | このチャンネルで川柳検出を再開 |
| `/rank` | サーバー内の川柳ランキングを表示 |
| `/delete` | 自分の川柳を選んで削除 |
| `/detect on` | 自分の川柳検出を有効化 |
| `/detect off` | 自分の川柳検出を無効化 |
| `/detect status` | 川柳検出の状態を確認 |
| `/detect ban` | 管理者がユーザーの検出を無効化 |
| `/detect unban` | 管理者がユーザーの検出を再有効化 |
| `/detect list` | 検出無効ユーザーの一覧を表示 |
| `/channel` | チャンネルタイプごとの検出設定 |
| `/doctor` | Botの動作状況を確認 |
| `/contact` | Bot管理者へ問い合わせ |
| `/admin stats` | 管理者向け統計 |
| `/admin backup` | 管理者向けバックアップ |
| `/admin contact-message` | 問い合わせへの返信 |

メッセージで `詠め` と送ると保存済みの川柳から一句作り、`詠むな` と送ると直前の川柳を表示します。

## 🔎 こんな人向け

- Discordサーバーに**川柳Bot / 5-7-5 Bot**を入れたい
- 普段の会話から偶然できた川柳を拾いたい
- DiscordのネタBotを簡単に導入したい
- Rust製Discord BotをPythonから起動したい
- サーバー内ランキングで盛り上げたい

## ❓ FAQ

### Discordの会話から自動で川柳を見つけられますか？

はい。Botが閲覧できるメッセージを対象に判定し、川柳として検出した発言に反応します。

### Pythonから起動できますか？

はい。`pip install senryu-bot` のあと、`from senryu_bot import run` で起動できます。

### 検出を止めることはできますか？

できます。ユーザー単位・チャンネル単位で検出を制御するためのコマンドがあります。

## 🤖 DiscordネタBotシリーズ

| Bot | 内容 |
|---|---|
| [meigen-bot](https://github.com/dtmpm3485/meigen-bot) | Discordの会話から名言・迷言を自動検出 |
| **senryu-bot** | 会話から川柳を自動検出 |
| [yaju-bot](https://github.com/dtmpm3485/yaju-bot) | Discordの会話に低確率で乱入するネタBot |

## ⭐ 気に入ったら

面白かった・役に立った場合は、GitHubの **Star ⭐** を付けてもらえると開発の励みになります。

Issue・改善案・バグ報告も歓迎です。

## License

MIT License
