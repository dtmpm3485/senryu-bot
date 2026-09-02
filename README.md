# senryu-bot

## インストール

```bash
pip install senryu-bot
```

## 使い方

```python
from senryu_bot import run

run("DISCORD_BOT_TOKEN")
```

## コマンド一覧

- `/mute` - このチャンネルで川柳検出を停止
- `/unmute` - このチャンネルで川柳検出を再開
- `/rank` - サーバー内の川柳ランキングを表示
- `/delete` - 自分の川柳を選んで削除
- `/detect on` - 自分の川柳検出を有効化
- `/detect off` - 自分の川柳検出を無効化
- `/detect status` - 川柳検出の状態を確認
- `/detect ban` - 管理者がユーザーの検出を無効化
- `/detect unban` - 管理者がユーザーの検出を再有効化
- `/detect list` - 検出無効ユーザーの一覧を表示
- `/channel` - チャンネルタイプごとの検出設定
- `/doctor` - Botの動作状況を確認
- `/contact` - Bot管理者へ問い合わせ
- `/admin stats` - 管理者向け統計
- `/admin backup` - 管理者向けバックアップ
- `/admin contact-message` - 問い合わせへの返信

メッセージで `詠め` と送ると保存済みの川柳から一句作り、`詠むな` と送ると直前の川柳を表示します。
