# Hakuren-Bot

note.comとYouTubeの特定アカウントの新規投稿を検知してDiscordにWebhookで通知する，身内鯖用Bot．

## Docker

```
cp .env.example .env   # DISCORD_WEBHOOK_URLを埋める
docker compose up -d --build
```

監視対象は`config.docker.toml`の`[[watch]]`と`[[youtube]]`で指定する．stateは`data/`に残るので再起動しても再通知しない．

## Local

```
cp config.example.toml config.toml   # urlnameとwebhook_urlを埋める
cargo run --release
```

初回は既存記事を既読にするだけで通知せず，以降の新規投稿のみ通知する．
