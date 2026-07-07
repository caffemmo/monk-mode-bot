use std::{env, str::FromStr, sync::Arc, time::Duration as StdDuration};

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use teloxide::{
    payloads::{AnswerCallbackQuerySetters, EditMessageReplyMarkupSetters, SendMessageSetters},
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};
use tokio::time;

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    owner_id: i64,
    tz: Tz,
    daily_summary_time: String,
    weekly_summary_time: String,
}

#[derive(Clone, Copy)]
struct TaskDef {
    id: &'static str,
    title: &'static str,
    points: i64,
}

#[derive(Clone, Copy)]
struct SectionDef {
    id: &'static str,
    time: &'static str,
    title: &'static str,
    intro: &'static str,
    tasks: &'static [TaskDef],
}

const MORNING_TASKS: &[TaskDef] = &[
    TaskDef { id: "water", title: "Uống 500ml nước", points: 5 },
    TaskDef { id: "blanket", title: "Gấp chăn", points: 5 },
    TaskDef { id: "brush", title: "Đánh răng", points: 5 },
    TaskDef { id: "face", title: "Rửa mặt", points: 5 },
    TaskDef { id: "window", title: "Mở cửa sổ", points: 5 },
    TaskDef { id: "breath", title: "Hít thở 5 phút", points: 10 },
];

const BREAKFAST_TASKS: &[TaskDef] = &[
    TaskDef { id: "protein", title: "Ăn đủ protein", points: 5 },
    TaskDef { id: "carb", title: "Ăn đủ tinh bột", points: 5 },
    TaskDef { id: "fruit", title: "Ăn trái cây", points: 5 },
];

const CARDIO_TASKS: &[TaskDef] = &[
    TaskDef { id: "cardio", title: "Cardio 20-30 phút", points: 15 },
    TaskDef { id: "no_phone", title: "Không lướt điện thoại khi tập", points: 5 },
];

const PLAN_TASKS: &[TaskDef] = &[
    TaskDef { id: "top3", title: "Viết 3 việc quan trọng nhất", points: 10 },
];

const DEEP_WORK_TASKS: &[TaskDef] = &[
    TaskDef { id: "pomodoro1", title: "Pomodoro 1", points: 10 },
    TaskDef { id: "pomodoro2", title: "Pomodoro 2", points: 10 },
    TaskDef { id: "phone_away", title: "Điện thoại để xa", points: 10 },
];

const LUNCH_TASKS: &[TaskDef] = &[
    TaskDef { id: "lunch", title: "Ăn trưa không trước máy tính", points: 10 },
];

const NAP_TASKS: &[TaskDef] = &[
    TaskDef { id: "nap", title: "Ngủ 30-45 phút", points: 10 },
];

const MMO_TASKS: &[TaskDef] = &[
    TaskDef { id: "mmo", title: "Tiếp tục MMO", points: 20 },
    TaskDef { id: "hourly_break", title: "Mỗi 1 giờ đứng dậy/uống nước", points: 10 },
];

const GYM_TASKS: &[TaskDef] = &[
    TaskDef { id: "train", title: "Tập nghiêm túc", points: 20 },
    TaskDef { id: "no_tiktok", title: "Không TikTok giữa các set", points: 10 },
    TaskDef { id: "stretch", title: "Giãn cơ sau tập", points: 10 },
];

const DINNER_TASKS: &[TaskDef] = &[
    TaskDef { id: "dinner", title: "Ăn tối đủ protein và uống nước", points: 10 },
];

const EVENING_TASKS: &[TaskDef] = &[
    TaskDef { id: "relax", title: "Giải trí có ý thức", points: 5 },
    TaskDef { id: "no_doom", title: "Không lướt vô thức/bình luận tiêu cực", points: 10 },
    TaskDef { id: "no_stalk", title: "Không stalk cô ấy", points: 20 },
];

const STUDY_TASKS: &[TaskDef] = &[
    TaskDef { id: "study60", title: "Học ít nhất 60 phút", points: 20 },
    TaskDef { id: "one_topic", title: "Chỉ học một chủ đề chính", points: 5 },
];

const LATE_WORK_TASKS: &[TaskDef] = &[
    TaskDef { id: "work_or_book", title: "Làm việc nếu còn việc, không thì đọc sách", points: 10 },
];

const READING_TASKS: &[TaskDef] = &[
    TaskDef { id: "read20", title: "Đọc sách 20 phút", points: 15 },
];

const JOURNAL_TASKS: &[TaskDef] = &[
    TaskDef { id: "journal", title: "Viết journal", points: 20 },
];

const MEDITATION_TASKS: &[TaskDef] = &[
    TaskDef { id: "meditate", title: "Thiền 10 phút", points: 10 },
];

const SLEEP_TASKS: &[TaskDef] = &[
    TaskDef { id: "shower", title: "Tắm và chuẩn bị ngủ", points: 5 },
    TaskDef { id: "sleep", title: "Ngủ lúc 01:00", points: 20 },
];

const SECTIONS: &[SectionDef] = &[
    SectionDef { id: "morning", time: "08:00", title: "🌅 08:00 THỨC DẬY", intro: "30 phút đầu tuyệt đối không mạng xã hội.", tasks: MORNING_TASKS },
    SectionDef { id: "breakfast", time: "08:15", title: "🍳 08:15 ĂN SÁNG", intro: "Không bỏ bữa. Không chỉ uống cà phê.", tasks: BREAKFAST_TASKS },
    SectionDef { id: "cardio", time: "08:45", title: "🚶 08:45 CARDIO", intro: "Đi bộ, chạy hoặc đạp xe.", tasks: CARDIO_TASKS },
    SectionDef { id: "plan", time: "09:15", title: "📒 09:15 LẬP KẾ HOẠCH", intro: "Chỉ viết 3 việc quan trọng nhất.", tasks: PLAN_TASKS },
    SectionDef { id: "deep", time: "09:30", title: "💻 09:30 DEEP WORK", intro: "50 phút làm, 10 phút nghỉ. Điện thoại để xa.", tasks: DEEP_WORK_TASKS },
    SectionDef { id: "lunch", time: "12:00", title: "🍱 12:00 ĂN TRƯA", intro: "Không ăn trước máy tính.", tasks: LUNCH_TASKS },
    SectionDef { id: "nap", time: "12:30", title: "😴 12:30 NGỦ", intro: "Ngủ ngắn 30-45 phút.", tasks: NAP_TASKS },
    SectionDef { id: "mmo", time: "13:30", title: "💻 13:30 MMO", intro: "Mỗi 1 tiếng đứng dậy, đi bộ, uống nước 5 phút.", tasks: MMO_TASKS },
    SectionDef { id: "gym", time: "17:00", title: "🏋️ 17:00 GYM", intro: "Tập nghiêm túc. Không bấm điện thoại.", tasks: GYM_TASKS },
    SectionDef { id: "dinner", time: "18:45", title: "🍗 18:45 ĂN TỐI", intro: "Ăn đủ protein và uống nước.", tasks: DINNER_TASKS },
    SectionDef { id: "evening", time: "19:30", title: "🎮 19:30 GIẢI TRÍ", intro: "Được giải trí, nhưng không stalk và không lướt vô thức.", tasks: EVENING_TASKS },
    SectionDef { id: "study", time: "20:30", title: "📚 20:30 HỌC", intro: "Ít nhất 1 tiếng. Một chủ đề thôi.", tasks: STUDY_TASKS },
    SectionDef { id: "latework", time: "22:00", title: "💼 22:00 LÀM VIỆC / ĐỌC SÁCH", intro: "Nếu còn việc thì làm, không thì đọc sách.", tasks: LATE_WORK_TASKS },
    SectionDef { id: "reading", time: "23:30", title: "📖 23:30 ĐỌC SÁCH", intro: "Đọc 20 phút.", tasks: READING_TASKS },
    SectionDef { id: "journal", time: "00:00", title: "📓 00:00 JOURNAL", intro: "Viết thật ngắn nhưng thật thật.", tasks: JOURNAL_TASKS },
    SectionDef { id: "meditation", time: "00:20", title: "🧘 00:20 THIỀN", intro: "10 phút im lặng.", tasks: MEDITATION_TASKS },
    SectionDef { id: "sleep", time: "00:40", title: "🚿 00:40 CHUẨN BỊ NGỦ", intro: "Tắm, tắt màn hình, ngủ lúc 01:00.", tasks: SLEEP_TASKS },
];

const IMPORTANT_TASK_IDS: &[&str] = &[
    "top3",
    "mmo",
    "train",
    "no_stalk",
    "study60",
    "read20",
    "journal",
    "sleep",
];

const EXCUSE_REASONS: &[(&str, &str)] = &[
    ("tired", "Mệt thật"),
    ("lazy", "Lười"),
    ("busy", "Bận"),
    ("sad", "Buồn"),
    ("no_reason", "Không lý do"),
];

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let token = env::var("BOT_TOKEN").context("BOT_TOKEN is required")?;
    let owner_id = env::var("OWNER_TELEGRAM_ID")
        .context("OWNER_TELEGRAM_ID is required")?
        .parse::<i64>()
        .context("OWNER_TELEGRAM_ID must be a number")?;
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://monk_mode.db".to_string());
    let tz = env::var("TIMEZONE")
        .unwrap_or_else(|_| "Asia/Bangkok".to_string())
        .parse::<Tz>()
        .context("invalid TIMEZONE")?;

    let options = SqliteConnectOptions::from_str(&db_url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;

    let state = Arc::new(AppState {
        pool,
        owner_id,
        tz,
        daily_summary_time: env::var("DAILY_SUMMARY_TIME").unwrap_or_else(|_| "00:55".to_string()),
        weekly_summary_time: env::var("WEEKLY_SUMMARY_TIME").unwrap_or_else(|_| "21:30".to_string()),
    });

    let bot = Bot::new(token);
    bot.set_my_commands(vec![
        teloxide::types::BotCommand::new("start", "Bắt đầu Monk Mode"),
        teloxide::types::BotCommand::new("today", "Tiến độ hôm nay"),
        teloxide::types::BotCommand::new("week", "Tổng kết 7 ngày"),
        teloxide::types::BotCommand::new("journal", "Viết journal"),
        teloxide::types::BotCommand::new("plan", "Viết 3 việc quan trọng"),
        teloxide::types::BotCommand::new("urge", "Đang nhớ cô ấy"),
        teloxide::types::BotCommand::new("help", "Hướng dẫn"),
    ])
    .await?;

    tokio::spawn(schedule_loop(bot.clone(), state.clone()));

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS monk_tasks (
            date TEXT NOT NULL,
            task_id TEXT NOT NULL,
            section_id TEXT NOT NULL,
            title TEXT NOT NULL,
            points INTEGER NOT NULL,
            completed INTEGER NOT NULL DEFAULT 0,
            completed_at TEXT,
            PRIMARY KEY (date, task_id)
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS monk_reminders (
            date TEXT NOT NULL,
            reminder_key TEXT NOT NULL,
            sent_at TEXT NOT NULL,
            PRIMARY KEY (date, reminder_key)
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS monk_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS monk_journals (
            date TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS monk_priorities (
            date TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS monk_sessions (
            user_id INTEGER PRIMARY KEY,
            state TEXT NOT NULL,
            date TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS monk_urges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            kind TEXT NOT NULL,
            note TEXT,
            created_at TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS monk_excuses (
            date TEXT NOT NULL,
            task_id TEXT NOT NULL,
            reason TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (date, task_id)
        )"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn handle_message(bot: Bot, msg: Message, state: Arc<AppState>) -> HandlerResult {
    let Some(user) = msg.from() else {
        return Ok(());
    };
    if user.id.0 as i64 != state.owner_id {
        return Ok(());
    }

    let chat_id = msg.chat.id;
    let text = msg.text().unwrap_or("").trim();
    if text.is_empty() {
        return Ok(());
    }

    if !text.starts_with('/') {
        if let Some((session, date)) = active_session(&state.pool, state.owner_id).await? {
            match session.as_str() {
                "journal" => {
                    save_journal(&state.pool, &date, text).await?;
                    mark_task_done(&state.pool, &date, "journal").await?;
                    clear_session(&state.pool, state.owner_id).await?;
                    bot.send_message(chat_id, format!("✅ Đã lưu journal cho ngày {date}.")).await?;
                    return Ok(());
                }
                "plan" => {
                    save_priorities(&state.pool, &date, text).await?;
                    mark_task_done(&state.pool, &date, "top3").await?;
                    clear_session(&state.pool, state.owner_id).await?;
                    bot.send_message(chat_id, format!("✅ Đã lưu 3 việc quan trọng cho ngày {date}.")).await?;
                    return Ok(());
                }
                "tomorrow_plan" => {
                    ensure_tasks_for_section(&state.pool, &date, section_by_id("plan").unwrap()).await?;
                    save_priorities(&state.pool, &date, text).await?;
                    mark_task_done(&state.pool, &date, "top3").await?;
                    clear_session(&state.pool, state.owner_id).await?;
                    bot.send_message(chat_id, format!("✅ Đã lưu kế hoạch cho ngày mai ({date}). Sáng mai bot sẽ nhắc lại.")).await?;
                    return Ok(());
                }
                _ => {}
            }
        }
    }

    match command_name(text) {
        "/start" | "/monk_start" => {
            let date = monk_date(&state);
            ensure_start_date(&state.pool, &date).await?;
            ensure_all_tasks_for_date(&state.pool, &date).await?;
            bot.send_message(chat_id, welcome_text(&state, &date).await?)
                .reply_markup(main_menu_keyboard())
                .await?;
        }
        "/today" | "/monk" | "/monk_today" => {
            let date = monk_date(&state);
            ensure_all_tasks_for_date(&state.pool, &date).await?;
            bot.send_message(chat_id, daily_summary_text(&state.pool, &date, &state).await?)
                .reply_markup(today_keyboard(&date))
                .await?;
        }
        "/week" | "/monk_week" => {
            let date = monk_date(&state);
            bot.send_message(chat_id, weekly_summary_text(&state.pool, &date).await?).await?;
        }
        "/journal" | "/monk_journal" => {
            let date = monk_date(&state);
            ensure_tasks_for_section(&state.pool, &date, section_by_id("journal").unwrap()).await?;
            set_session(&state.pool, state.owner_id, "journal", &date).await?;
            bot.send_message(chat_id, journal_prompt(&date)).await?;
        }
        "/plan" => {
            let date = monk_date(&state);
            ensure_tasks_for_section(&state.pool, &date, section_by_id("plan").unwrap()).await?;
            set_session(&state.pool, state.owner_id, "plan", &date).await?;
            bot.send_message(chat_id, plan_prompt(&date)).await?;
        }
        "/urge" => {
            let date = monk_date(&state);
            bot.send_message(chat_id, urge_text())
                .reply_markup(urge_keyboard(&date))
                .await?;
        }
        "/help" => {
            bot.send_message(chat_id, help_text()).await?;
        }
        _ => {
            bot.send_message(chat_id, "Lệnh không rõ. Gõ /help để xem hướng dẫn.").await?;
        }
    }

    Ok(())
}

async fn handle_callback(bot: Bot, q: CallbackQuery, state: Arc<AppState>) -> HandlerResult {
    if q.from.id.0 as i64 != state.owner_id {
        bot.answer_callback_query(q.id).text("Không có quyền.").await?;
        return Ok(());
    }
    let Some(data) = q.data.clone() else {
        return Ok(());
    };
    let Some(message) = q.message.as_ref() else {
        return Ok(());
    };
    let chat_id = message.chat().id;

    if let Some(rest) = data.strip_prefix("t|") {
        let parts = rest.split('|').collect::<Vec<_>>();
        if parts.len() != 3 {
            bot.answer_callback_query(q.id).text("Callback lỗi.").await?;
            return Ok(());
        }
        let date = parts[0];
        let section_id = parts[1];
        let task_id = parts[2];
        toggle_task(&state.pool, date, task_id).await?;
        bot.answer_callback_query(q.id).text("Đã cập nhật.").await?;
        if let Some(section) = section_by_id(section_id) {
            bot.edit_message_reply_markup(chat_id, message.id())
                .reply_markup(section_keyboard(&state.pool, date, section).await?)
                .await?;
        }
    } else if data == "today" {
        let date = monk_date(&state);
        bot.answer_callback_query(q.id).await?;
        bot.send_message(chat_id, daily_summary_text(&state.pool, &date, &state).await?)
            .reply_markup(today_keyboard(&date))
            .await?;
    } else if data == "week" {
        let date = monk_date(&state);
        bot.answer_callback_query(q.id).await?;
        bot.send_message(chat_id, weekly_summary_text(&state.pool, &date).await?).await?;
    } else if data == "journal" {
        let date = monk_date(&state);
        ensure_tasks_for_section(&state.pool, &date, section_by_id("journal").unwrap()).await?;
        set_session(&state.pool, state.owner_id, "journal", &date).await?;
        bot.answer_callback_query(q.id).await?;
        bot.send_message(chat_id, journal_prompt(&date)).await?;
    } else if data == "plan" {
        let date = monk_date(&state);
        ensure_tasks_for_section(&state.pool, &date, section_by_id("plan").unwrap()).await?;
        set_session(&state.pool, state.owner_id, "plan", &date).await?;
        bot.answer_callback_query(q.id).await?;
        bot.send_message(chat_id, plan_prompt(&date)).await?;
    } else if data == "urge" {
        let date = monk_date(&state);
        bot.answer_callback_query(q.id).await?;
        bot.send_message(chat_id, urge_text())
            .reply_markup(urge_keyboard(&date))
            .await?;
    } else if let Some(raw_kind) = data.strip_prefix("urge|") {
        let date = monk_date(&state);
        let kind = raw_kind.split('|').next().unwrap_or(raw_kind);
        save_urge(&state.pool, &date, kind).await?;
        if kind == "stalk" {
            mark_task_undone(&state.pool, &date, "no_stalk").await?;
        } else if kind == "pass" {
            mark_task_done(&state.pool, &date, "no_stalk").await?;
        }
        bot.answer_callback_query(q.id).text("Đã ghi nhận.").await?;
        bot.send_message(chat_id, "Ghi nhận xong. Quay lại đường ray ngay bây giờ.").await?;
    } else if let Some(rest) = data.strip_prefix("excuse|") {
        let parts = rest.split('|').collect::<Vec<_>>();
        if parts.len() != 3 {
            bot.answer_callback_query(q.id).text("Callback lỗi.").await?;
            return Ok(());
        }
        let date = parts[0];
        let task_id = parts[1];
        let reason = parts[2];
        save_excuse(&state.pool, date, task_id, reason).await?;
        bot.answer_callback_query(q.id)
            .text(format!("Đã ghi lý do: {}", excuse_reason_label(reason)))
            .await?;
        bot.edit_message_reply_markup(chat_id, message.id())
            .reply_markup(InlineKeyboardMarkup::new(Vec::<Vec<InlineKeyboardButton>>::new()))
            .await?;
    }

    Ok(())
}

async fn schedule_loop(bot: Bot, state: Arc<AppState>) {
    let mut ticker = time::interval(StdDuration::from_secs(30));
    loop {
        ticker.tick().await;
        if let Err(err) = schedule_tick(&bot, &state).await {
            tracing::error!("schedule tick failed: {err:?}");
        }
    }
}

async fn schedule_tick(bot: &Bot, state: &AppState) -> Result<()> {
    let now = Utc::now().with_timezone(&state.tz);
    let hhmm = format!("{:02}:{:02}", now.hour(), now.minute());
    let date = monk_date_from_local(now);
    ensure_start_date(&state.pool, &date).await?;

    for section in SECTIONS {
        if section.time == hhmm {
            let key = format!("section:{}", section.id);
            if mark_reminder_sent(&state.pool, &date, &key).await? {
                ensure_tasks_for_section(&state.pool, &date, section).await?;
                bot.send_message(ChatId(state.owner_id), section_text(&date, section))
                    .reply_markup(section_keyboard(&state.pool, &date, section).await?)
                    .await?;
                if section.id == "journal" {
                    set_session(&state.pool, state.owner_id, "journal", &date).await?;
                    bot.send_message(ChatId(state.owner_id), journal_prompt(&date)).await?;
                }
                if section.id == "plan" {
                    set_session(&state.pool, state.owner_id, "plan", &date).await?;
                    bot.send_message(ChatId(state.owner_id), plan_prompt(&date)).await?;
                }
                if section.id == "morning" {
                    send_morning_plan_reminder(bot, state, &date).await?;
                }
            }
        }
    }

    if hhmm == state.daily_summary_time {
        let key = "daily_summary";
        if mark_reminder_sent(&state.pool, &date, key).await? {
            ensure_all_tasks_for_date(&state.pool, &date).await?;
            bot.send_message(ChatId(state.owner_id), daily_summary_text(&state.pool, &date, state).await?)
                .reply_markup(today_keyboard(&date))
                .await?;
            send_excuse_prompts(bot, state, &date).await?;
            let tomorrow = next_date(&date)?;
            ensure_tasks_for_section(&state.pool, &tomorrow, section_by_id("plan").unwrap()).await?;
            set_session(&state.pool, state.owner_id, "tomorrow_plan", &tomorrow).await?;
            bot.send_message(ChatId(state.owner_id), tomorrow_plan_prompt(&date, &tomorrow))
                .await?;
        }
    }

    if now.weekday() == Weekday::Sun && hhmm == state.weekly_summary_time {
        let key = "weekly_summary";
        if mark_reminder_sent(&state.pool, &date, key).await? {
            bot.send_message(ChatId(state.owner_id), weekly_summary_text(&state.pool, &date).await?)
                .await?;
        }
    }

    Ok(())
}

async fn welcome_text(state: &AppState, date: &str) -> Result<String> {
    let day = day_number(&state.pool, date).await?;
    Ok(format!(
        "🧘 MONK MODE - 90 NGÀY\n\nNgày {day}/90 đã bắt đầu.\n\nKhông vì ai khác.\nNhưng nếu một ngày cô ấy nhìn lại, cô ấy sẽ thấy một người đàn ông hoàn toàn khác."
    ))
}

fn help_text() -> &'static str {
    "Lệnh:\n/start - bắt đầu\n/today - tiến độ hôm nay\n/week - tổng kết tuần\n/plan - ghi 3 việc quan trọng\n/journal - ghi journal\n/urge - đang nhớ cô ấy\n\nBot chỉ trả lời owner Telegram ID trong .env."
}

fn section_text(date: &str, section: &SectionDef) -> String {
    format!("{}\nNgày: {}\n\n{}\n\nTick khi xong:", section.title, date, section.intro)
}

fn journal_prompt(date: &str) -> String {
    format!(
        "📓 Journal ngày {date}\n\nTrả lời một tin nhắn với 5 dòng:\n1. Hôm nay mình làm tốt gì?\n2. Sai gì?\n3. Học được gì?\n4. Biết ơn điều gì?\n5. Mai làm gì?"
    )
}

fn plan_prompt(date: &str) -> String {
    format!("📒 Kế hoạch ngày {date}\n\nGửi 3 việc quan trọng nhất hôm nay, mỗi việc một dòng.")
}

fn tomorrow_plan_prompt(today: &str, tomorrow: &str) -> String {
    format!(
        "🌙 Cuối ngày {today}\n\nViết 3 kế hoạch muốn làm cho ngày mai ({tomorrow}).\nMỗi việc một dòng.\n\nSáng mai bot sẽ gửi lại để bạn không bắt đầu ngày mới trong mơ hồ."
    )
}

fn urge_text() -> &'static str {
    "🚨 ĐANG NHỚ CÔ ẤY\n\nDừng lại 10 phút.\n\nKhông mở story.\nKhông đọc tin cũ.\nKhông nhắn khi đang nhớ.\n\nLàm 1 trong 3:\n🏋️ Gym\n🚶 Đi bộ\n📓 Viết 5 dòng\n\nChọn kết quả bên dưới."
}

fn main_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("📋 Hôm nay", "today"),
            InlineKeyboardButton::callback("📊 Tuần này", "week"),
        ],
        vec![
            InlineKeyboardButton::callback("📒 Kế hoạch", "plan"),
            InlineKeyboardButton::callback("📓 Journal", "journal"),
        ],
        vec![InlineKeyboardButton::callback("🚨 Đang nhớ cô ấy", "urge")],
    ])
}

fn today_keyboard(_date: &str) -> InlineKeyboardMarkup {
    main_menu_keyboard()
}

fn urge_keyboard(date: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("✅ Tôi đã vượt qua", format!("urge|pass|{date}"))],
        vec![InlineKeyboardButton::callback("❌ Tôi đã stalk", format!("urge|stalk|{date}"))],
    ])
}

fn excuse_keyboard(date: &str, task_id: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(
        EXCUSE_REASONS
            .iter()
            .map(|(reason, label)| {
                vec![InlineKeyboardButton::callback(
                    (*label).to_string(),
                    format!("excuse|{date}|{task_id}|{reason}"),
                )]
            })
            .collect::<Vec<_>>(),
    )
}

async fn section_keyboard(pool: &SqlitePool, date: &str, section: &SectionDef) -> Result<InlineKeyboardMarkup> {
    let mut rows = Vec::new();
    for task in section.tasks {
        let done = task_completed(pool, date, task.id).await?;
        let icon = if done { "✅" } else { "⬜" };
        rows.push(vec![InlineKeyboardButton::callback(
            format!("{icon} {}", task.title),
            format!("t|{date}|{}|{}", section.id, task.id),
        )]);
    }
    rows.push(vec![InlineKeyboardButton::callback("📋 Xem tiến độ hôm nay", "today")]);
    rows.push(vec![InlineKeyboardButton::callback("🚨 Đang nhớ cô ấy", "urge")]);
    Ok(InlineKeyboardMarkup::new(rows))
}

async fn ensure_all_tasks_for_date(pool: &SqlitePool, date: &str) -> Result<()> {
    for section in SECTIONS {
        ensure_tasks_for_section(pool, date, section).await?;
    }
    Ok(())
}

async fn ensure_tasks_for_section(pool: &SqlitePool, date: &str, section: &SectionDef) -> Result<()> {
    for task in section.tasks {
        sqlx::query(
            "INSERT INTO monk_tasks (date, task_id, section_id, title, points, completed)
             VALUES (?, ?, ?, ?, ?, 0)
             ON CONFLICT(date, task_id) DO UPDATE SET
                section_id = excluded.section_id,
                title = excluded.title,
                points = excluded.points",
        )
        .bind(date)
        .bind(task.id)
        .bind(section.id)
        .bind(task.title)
        .bind(task.points)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn task_completed(pool: &SqlitePool, date: &str, task_id: &str) -> Result<bool> {
    let completed = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT completed FROM monk_tasks WHERE date = ? AND task_id = ?",
    )
    .bind(date)
    .bind(task_id)
    .fetch_optional(pool)
    .await?
    .flatten()
    .unwrap_or(0);
    Ok(completed != 0)
}

async fn toggle_task(pool: &SqlitePool, date: &str, task_id: &str) -> Result<()> {
    let done = task_completed(pool, date, task_id).await?;
    if done {
        mark_task_undone(pool, date, task_id).await
    } else {
        mark_task_done(pool, date, task_id).await
    }
}

async fn mark_task_done(pool: &SqlitePool, date: &str, task_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE monk_tasks SET completed = 1, completed_at = ? WHERE date = ? AND task_id = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(date)
    .bind(task_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_task_undone(pool: &SqlitePool, date: &str, task_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE monk_tasks SET completed = 0, completed_at = NULL WHERE date = ? AND task_id = ?",
    )
    .bind(date)
    .bind(task_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn daily_summary_text(pool: &SqlitePool, date: &str, state: &AppState) -> Result<String> {
    ensure_all_tasks_for_date(pool, date).await?;
    let rows = sqlx::query(
        "SELECT task_id, title, points, completed FROM monk_tasks WHERE date = ? ORDER BY section_id, task_id",
    )
    .bind(date)
    .fetch_all(pool)
    .await?;
    let total_points: i64 = rows.iter().map(|row| row.get::<i64, _>("points")).sum();
    let done_points: i64 = rows
        .iter()
        .filter(|row| row.get::<i64, _>("completed") != 0)
        .map(|row| row.get::<i64, _>("points"))
        .sum();
    let score = if total_points > 0 { done_points * 100 / total_points } else { 0 };
    let done_count = rows.iter().filter(|row| row.get::<i64, _>("completed") != 0).count();
    let day = day_number(pool, date).await?;
    let journal = sqlx::query_scalar::<_, Option<String>>("SELECT content FROM monk_journals WHERE date = ?")
        .bind(date)
        .fetch_optional(pool)
        .await?
        .flatten();
    let priorities = sqlx::query_scalar::<_, Option<String>>("SELECT content FROM monk_priorities WHERE date = ?")
        .bind(date)
        .fetch_optional(pool)
        .await?
        .flatten();

    let mut lines = vec![
        format!("🌙 Tổng kết ngày {day}/90"),
        format!("Ngày: {date}"),
        String::new(),
        format!("Điểm: {score}/100"),
        format!("Hoàn thành: {done_count}/{}", rows.len()),
        String::new(),
    ];

    for row in rows {
        let icon = if row.get::<i64, _>("completed") != 0 { "✅" } else { "❌" };
        lines.push(format!("{icon} {}", row.get::<String, _>("title")));
    }

    if let Some(priorities) = priorities {
        lines.push(String::new());
        lines.push(format!("📒 3 việc:\n{priorities}"));
    }
    if journal.is_some() {
        lines.push(String::new());
        lines.push("📓 Journal: đã lưu".to_string());
    }

    let comment = if score >= 85 {
        "Nhận xét: Hôm nay rất tốt. Kỷ luật đang thắng cảm xúc."
    } else if score >= 60 {
        "Nhận xét: Chưa hoàn hảo, nhưng bạn vẫn đang ở trên đường ray."
    } else {
        "Nhận xét: Hôm nay yếu. Đừng tự đánh mình, ngày mai quay lại nhịp."
    };
    lines.push(String::new());
    lines.push(comment.to_string());
    lines.push(format!("Giờ tổng kết hằng ngày: {}", state.daily_summary_time));

    Ok(lines.join("\n"))
}

async fn weekly_summary_text(pool: &SqlitePool, date: &str) -> Result<String> {
    let end = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
    let start = end - Duration::days(6);
    let mut lines = vec![
        "📊 Monk Mode - Tổng kết 7 ngày".to_string(),
        format!("{start} -> {end}"),
        String::new(),
    ];

    let mut total_score = 0;
    let mut days = 0;
    for i in 0..7 {
        let d = (start + Duration::days(i)).format("%Y-%m-%d").to_string();
        ensure_all_tasks_for_date(pool, &d).await?;
        let (done, total) = score_for_date(pool, &d).await?;
        let score = if total > 0 { done * 100 / total } else { 0 };
        total_score += score;
        days += 1;
        lines.push(format!("{d}: {score}/100"));
    }

    let avg = if days > 0 { total_score / days } else { 0 };
    let urge_rows = sqlx::query(
        "SELECT kind, COUNT(1) AS count FROM monk_urges WHERE date >= ? AND date <= ? GROUP BY kind",
    )
    .bind(start.format("%Y-%m-%d").to_string())
    .bind(end.format("%Y-%m-%d").to_string())
    .fetch_all(pool)
    .await?;
    let excuse_rows = sqlx::query(
        "SELECT reason, COUNT(1) AS count FROM monk_excuses WHERE date >= ? AND date <= ? GROUP BY reason ORDER BY count DESC",
    )
    .bind(start.format("%Y-%m-%d").to_string())
    .bind(end.format("%Y-%m-%d").to_string())
    .fetch_all(pool)
    .await?;

    lines.push(String::new());
    lines.push(format!("Điểm trung bình: {avg}/100"));
    for row in urge_rows {
        let kind = row.get::<String, _>("kind");
        lines.push(format!("{}: {}", urge_kind_label(&kind), row.get::<i64, _>("count")));
    }
    if !excuse_rows.is_empty() {
        lines.push(String::new());
        lines.push("Anti-Excuse:".to_string());
        for row in excuse_rows {
            let reason = row.get::<String, _>("reason");
            lines.push(format!(
                "{}: {}",
                excuse_reason_label(&reason),
                row.get::<i64, _>("count")
            ));
        }
    }
    lines.push(String::new());
    lines.push("Kết luận: Đừng cần hoàn hảo. Cần quay lại mỗi ngày.".to_string());
    Ok(lines.join("\n"))
}

async fn send_morning_plan_reminder(bot: &Bot, state: &AppState, date: &str) -> Result<()> {
    if let Some(plan) = load_priorities(&state.pool, date).await? {
        bot.send_message(
            ChatId(state.owner_id),
            format!("📒 Kế hoạch bạn đã hứa cho hôm nay ({date}):\n\n{plan}\n\nĐừng thương lượng với cảm xúc. Làm từng việc một."),
        )
        .await?;
    }
    Ok(())
}

async fn send_excuse_prompts(bot: &Bot, state: &AppState, date: &str) -> Result<()> {
    let missed = missed_important_tasks(&state.pool, date).await?;
    for (task_id, title) in missed {
        bot.send_message(
            ChatId(state.owner_id),
            format!("🧱 Anti-Excuse\n\nBạn chưa hoàn thành: {title}\n\nLý do thật là gì?"),
        )
        .reply_markup(excuse_keyboard(date, &task_id))
        .await?;
    }
    Ok(())
}

fn urge_kind_label(kind: &str) -> &str {
    match kind {
        "pass" => "Đã vượt qua",
        "stalk" => "Đã stalk",
        _ => kind,
    }
}

fn excuse_reason_label(reason: &str) -> &str {
    EXCUSE_REASONS
        .iter()
        .find_map(|(key, label)| (*key == reason).then_some(*label))
        .unwrap_or(reason)
}

async fn score_for_date(pool: &SqlitePool, date: &str) -> Result<(i64, i64)> {
    let row = sqlx::query(
        "SELECT
            COALESCE(SUM(CASE WHEN completed != 0 THEN points ELSE 0 END), 0) AS done,
            COALESCE(SUM(points), 0) AS total
         FROM monk_tasks WHERE date = ?",
    )
    .bind(date)
    .fetch_one(pool)
    .await?;
    Ok((row.get("done"), row.get("total")))
}

async fn save_journal(pool: &SqlitePool, date: &str, content: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO monk_journals (date, content, created_at, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(date) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
    )
    .bind(date)
    .bind(content)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

async fn save_priorities(pool: &SqlitePool, date: &str, content: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO monk_priorities (date, content, created_at, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(date) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
    )
    .bind(date)
    .bind(content)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

async fn load_priorities(pool: &SqlitePool, date: &str) -> Result<Option<String>> {
    let priorities = sqlx::query_scalar::<_, String>("SELECT content FROM monk_priorities WHERE date = ?")
        .bind(date)
        .fetch_optional(pool)
        .await?;
    Ok(priorities)
}

async fn missed_important_tasks(pool: &SqlitePool, date: &str) -> Result<Vec<(String, String)>> {
    let mut missed = Vec::new();
    for task_id in IMPORTANT_TASK_IDS {
        let row = sqlx::query(
            "SELECT title, completed FROM monk_tasks WHERE date = ? AND task_id = ?",
        )
        .bind(date)
        .bind(task_id)
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            continue;
        };
        if row.get::<i64, _>("completed") != 0 {
            continue;
        }
        let already_answered = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM monk_excuses WHERE date = ? AND task_id = ?",
        )
        .bind(date)
        .bind(task_id)
        .fetch_optional(pool)
        .await?
        .is_some();
        if !already_answered {
            missed.push(((*task_id).to_string(), row.get::<String, _>("title")));
        }
    }
    Ok(missed)
}

async fn save_excuse(pool: &SqlitePool, date: &str, task_id: &str, reason: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO monk_excuses (date, task_id, reason, created_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(date, task_id) DO UPDATE SET reason = excluded.reason, created_at = excluded.created_at",
    )
    .bind(date)
    .bind(task_id)
    .bind(reason)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

async fn save_urge(pool: &SqlitePool, date: &str, kind: &str) -> Result<()> {
    sqlx::query("INSERT INTO monk_urges (date, kind, created_at) VALUES (?, ?, ?)")
        .bind(date)
        .bind(kind)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
    Ok(())
}

async fn active_session(pool: &SqlitePool, user_id: i64) -> Result<Option<(String, String)>> {
    let row = sqlx::query("SELECT state, date FROM monk_sessions WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| (row.get("state"), row.get("date"))))
}

async fn set_session(pool: &SqlitePool, user_id: i64, session: &str, date: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO monk_sessions (user_id, state, date, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET state = excluded.state, date = excluded.date, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(session)
    .bind(date)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

async fn clear_session(pool: &SqlitePool, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM monk_sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn mark_reminder_sent(pool: &SqlitePool, date: &str, key: &str) -> Result<bool> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO monk_reminders (date, reminder_key, sent_at) VALUES (?, ?, ?)",
    )
    .bind(date)
    .bind(key)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

async fn ensure_start_date(pool: &SqlitePool, date: &str) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO monk_meta (key, value) VALUES ('start_date', ?)")
        .bind(date)
        .execute(pool)
        .await?;
    Ok(())
}

async fn day_number(pool: &SqlitePool, date: &str) -> Result<i64> {
    let start = sqlx::query_scalar::<_, String>("SELECT value FROM monk_meta WHERE key = 'start_date'")
        .fetch_optional(pool)
        .await?
        .unwrap_or_else(|| date.to_string());
    let start = NaiveDate::parse_from_str(&start, "%Y-%m-%d")?;
    let current = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
    Ok((current - start).num_days() + 1)
}

fn monk_date(state: &AppState) -> String {
    monk_date_from_local(Utc::now().with_timezone(&state.tz))
}

fn monk_date_from_local(now: chrono::DateTime<Tz>) -> String {
    let date = if now.hour() < 4 {
        now.date_naive() - Duration::days(1)
    } else {
        now.date_naive()
    };
    date.format("%Y-%m-%d").to_string()
}

fn next_date(date: &str) -> Result<String> {
    let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")? + Duration::days(1);
    Ok(date.format("%Y-%m-%d").to_string())
}

fn command_name(text: &str) -> &str {
    text.split_whitespace()
        .next()
        .unwrap_or("")
        .split('@')
        .next()
        .unwrap_or("")
}

fn section_by_id(id: &str) -> Option<&'static SectionDef> {
    SECTIONS.iter().find(|section| section.id == id)
}
