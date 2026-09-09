# senryu-bot

[![PyPI](https://img.shields.io/pypi/v/senryu-bot)](https://pypi.org/project/senryu-bot/)
[![Python](https://img.shields.io/pypi/pyversions/senryu-bot)](https://pypi.org/project/senryu-bot/)
[![License](https://img.shields.io/github/license/dtmpm3485/senryu-bot)](LICENSE)

Discordの会話から川柳を自動で検出するBotです。Bot本体と判定処理はRustで実装されており、Pythonから起動できます。

<img src="https://raw.githubusercontent.com/dtmpm3485/senryu-bot/main/assets/demo.jpg" alt="senryu-botの動作例" width="100%">

## 機能

- 通常の会話から川柳を自動検出
- サーバー内ランキング
- ユーザー単位の検出ON/OFF
- チャンネル単位の検出設定
- 保存した川柳の削除
- Botの状態確認
- 管理者向け統計、バックアップ、問い合わせ機能

## インストール

```bash
pip install -U senryu-bot
```

## 起動

```python
from senryu_bot import run

run("DISCORD_BOT_TOKEN")
```

## コマンド

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

## 関連リポジトリ

- [meigen-bot](https://github.com/dtmpm3485/meigen-bot) - Discordの会話から名言・迷言を検出するBot
- [yaju-bot](https://github.com/dtmpm3485/yaju-bot) - Discordの会話にランダムで乱入するBot

## License

MIT License
