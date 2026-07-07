# Monk Mode Bot

Telegram bot riêng cho Monk Mode 90 ngày.

## Chuẩn bị

1. Vào `@BotFather` và revoke token đã từng paste vào chat.
2. Tạo token mới.
3. Copy `.env.example` thành `.env`.
4. Điền:

```env
BOT_TOKEN=token_moi
OWNER_TELEGRAM_ID=telegram_id_của_bạn
DATABASE_URL=sqlite://monk_mode.db
TIMEZONE=Asia/Bangkok
```

## Chạy local/VPS

```bash
cargo run --release
```

## Lệnh Telegram

- `/start` - bắt đầu Monk Mode
- `/today` - xem tiến độ hôm nay
- `/week` - tổng kết 7 ngày gần nhất
- `/journal` - ghi journal hôm nay
- `/plan` - ghi 3 việc quan trọng
- `/urge` - protocol khi đang nhớ cô ấy
- `/help` - hướng dẫn

Bot chỉ phản hồi `OWNER_TELEGRAM_ID`.

## Systemd gợi ý

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
