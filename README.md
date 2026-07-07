# Monk Mode Bot

Telegram bot rieng cho Monk Mode 90 ngay.

## Chuan bi

1. Vao `@BotFather` va revoke token da tung paste vao chat.
2. Tao token moi.
3. Copy `.env.example` thanh `.env`.
4. Dien:

```env
BOT_TOKEN=token_moi
OWNER_TELEGRAM_ID=telegram_id_cua_ban
DATABASE_URL=sqlite://monk_mode.db
TIMEZONE=Asia/Bangkok
```

## Chay local/VPS

```bash
cargo run --release
```

## Lenh Telegram

- `/start` - bat dau Monk Mode
- `/today` - xem tien do hom nay
- `/week` - tong ket 7 ngay gan nhat
- `/journal` - ghi journal hom nay
- `/plan` - ghi 3 viec quan trong
- `/urge` - protocol khi dang nho co ay
- `/help` - huong dan

Bot chi phan hoi `OWNER_TELEGRAM_ID`.

## Systemd goi y

```ini
[Unit]
Description=Monk Mode Telegram Bot
After=network.target

[Service]
Type=simple
WorkingDirectory=/opt/monk-mode-bot
EnvironmentFile=/opt/monk-mode-bot/.env
ExecStart=/opt/monk-mode-bot/target/release/monk-mode-bot
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```
