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
    TaskDef { id: "water", title: "Uong 500ml nuoc", points: 5 },
    TaskDef { id: "blanket", title: "Gap chan", points: 5 },
    TaskDef { id: "brush", title: "Danh rang", points: 5 },
    TaskDef { id: "face", title: "Rua mat", points: 5 },
    TaskDef { id: "window", title: "Mo cua so", points: 5 },
    TaskDef { id: "breath", title: "Hit tho 5 phut", points: 10 },
];

const BREAKFAST_TASKS: &[TaskDef] = &[
    TaskDef { id: "protein", title: "An du protein", points: 5 },
    TaskDef { id: "carb", title: "An du tinh bot", points: 5 },
    TaskDef { id: "fruit", title: "An trai cay", points: 5 },
];

const CARDIO_TASKS: &[TaskDef] = &[
    TaskDef { id: "cardio", title: "Cardio 20-30 phut", points: 15 },
    TaskDef { id: "no_phone", title: "Khong luot dien thoai khi tap", points: 5 },
];

const PLAN_TASKS: &[TaskDef] = &[
    TaskDef { id: "top3", title: "Viet 3 viec quan trong nhat", points: 10 },
];

const DEEP_WORK_TASKS: &[TaskDef] = &[
    TaskDef { id: "pomodoro1", title: "Pomodoro 1", points: 10 },
    TaskDef { id: "pomodoro2", title: "Pomodoro 2", points: 10 },
    TaskDef { id: "phone_away", title: "Dien thoai de xa", points: 10 },
];

const LUNCH_TASKS: &[TaskDef] = &[
    TaskDef { id: "lunch", title: "An trua khong truoc may tinh", points: 10 },
];

const NAP_TASKS: &[TaskDef] = &[
    TaskDef { id: "nap", title: "Ngu 30-45 phut", points: 10 },
];

const MMO_TASKS: &[TaskDef] = &[
    TaskDef { id: "mmo", title: "Tiep tuc MMO", points: 20 },
    TaskDef { id: "hourly_break", title: "Moi 1 gio dung day/uong nuoc", points: 10 },
];

const GYM_TASKS: &[TaskDef] = &[
    TaskDef { id: "train", title: "Tap nghiem tuc", points: 20 },
    TaskDef { id: "no_tiktok", title: "Khong TikTok giua cac set", points: 10 },
    TaskDef { id: "stretch", title: "Gian co sau tap", points: 10 },
];

const DINNER_TASKS: &[TaskDef] = &[
    TaskDef { id: "dinner", title: "An toi du protein va uong nuoc", points: 10 },
];

const EVENING_TASKS: &[TaskDef] = &[
    TaskDef { id: "relax", title: "Giai tri co y thuc", points: 5 },
    TaskDef { id: "no_doom", title: "Khong luot vo thuc/binh luan tieu cuc", points: 10 },
    TaskDef { id: "no_stalk", title: "Khong stalk co ay", points: 20 },
];

const STUDY_TASKS: &[TaskDef] = &[
    TaskDef { id: "study60", title: "Hoc it nhat 60 phut", points: 20 },
    TaskDef { id: "one_topic", title: "Chi hoc mot chu de chinh", points: 5 },
];

const LATE_WORK_TASKS: &[TaskDef] = &[
    TaskDef { id: "work_or_book", title: "Lam viec neu con viec, khong thi doc sach", points: 10 },
];

const READING_TASKS: &[TaskDef] = &[
    TaskDef { id: "read20", title: "Doc sach 20 phut", points: 15 },
];

const JOURNAL_TASKS: &[TaskDef] = &[
    TaskDef { id: "journal", title: "Viet journal", points: 20 },
];

const MEDITATION_TASKS: &[TaskDef] = &[
    TaskDef { id: "meditate", title: "Thien 10 phut", points: 10 },
];

const SLEEP_TASKS: &[TaskDef] = &[
    TaskDef { id: "shower", title: "Tam va chuan bi ngu", points: 5 },
    TaskDef { id: "sleep", title: "Ngu luc 01:00", points: 20 },
];

const SECTIONS: &[SectionDef] = &[
    SectionDef { id: "morning", time: "08:00", title: "🌅 08:00 THUC DAY", intro: "30 phut dau tuyet doi khong mang xa hoi.", tasks: MORNING_TASKS },
    SectionDef { id: "breakfast", time: "08:15", title: "🍳 08:15 AN SANG", intro: "Khong bo bua. Khong chi uong ca phe.", tasks: BREAKFAST_TASKS },
    SectionDef { id: "cardio", time: "08:45", title: "🚶 08:45 CARDIO", intro: "Di bo, chay hoac dap xe.", tasks: CARDIO_TASKS },
    SectionDef { id: "plan", time: "09:15", title: "📒 09:15 LAP KE HOACH", intro: "Chi viet 3 viec quan trong nhat.", tasks: PLAN_TASKS },
    SectionDef { id: "deep", time: "09:30", title: "💻 09:30 DEEP WORK", intro: "50 phut lam, 10 phut nghi. Dien thoai de xa.", tasks: DEEP_WORK_TASKS },
    SectionDef { id: "lunch", time: "12:00", title: "🍱 12:00 AN TRUA", intro: "Khong an truoc may tinh.", tasks: LUNCH_TASKS },
    SectionDef { id: "nap", time: "12:30", title: "😴 12:30 NGU", intro: "Ngu ngan 30-45 phut.", tasks: NAP_TASKS },
    SectionDef { id: "mmo", time: "13:30", title: "💻 13:30 MMO", intro: "Moi 1 tieng dung day, di bo, uong nuoc 5 phut.", tasks: MMO_TASKS },
    SectionDef { id: "gym", time: "17:00", title: "🏋️ 17:00 GYM", intro: "Tap nghiem tuc. Khong bam dien thoai.", tasks: GYM_TASKS },
    SectionDef { id: "dinner", time: "18:45", title: "🍗 18:45 AN TOI", intro: "An du protein va uong nuoc.", tasks: DINNER_TASKS },
    SectionDef { id: "evening", time: "19:30", title: "🎮 19:30 GIAI TRI", intro: "Duoc giai tri, nhung khong stalk va khong luot vo thuc.", tasks: EVENING_TASKS },
    SectionDef { id: "study", time: "20:30", title: "📚 20:30 HOC", intro: "It nhat 1 tieng. Mot chu de thoi.", tasks: STUDY_TASKS },
    SectionDef { id: "latework", time: "22:00", title: "💼 22:00 LAM VIEC / DOC SACH", intro: "Neu con viec thi lam, khong thi doc sach.", tasks: LATE_WORK_TASKS },
    SectionDef { id: "reading", time: "23:30", title: "📖 23:30 DOC SACH", intro: "Doc 20 phut.", tasks: READING_TASKS },
    SectionDef { id: "journal", time: "00:00", title: "📓 00:00 JOURNAL", intro: "Viet that ngan nhung that that.", tasks: JOURNAL_TASKS },
    SectionDef { id: "meditation", time: "00:20", title: "🧘 00:20 THIEN", intro: "10 phut im lang.", tasks: MEDITATION_TASKS },
    SectionDef { id: "sleep", time: "00:40", title: "🚿 00:40 CHUAN BI NGU", intro: "Tam, tat man hinh, ngu luc 01:00.", tasks: SLEEP_TASKS },
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
        teloxide::types::BotCommand::new("start", "Bat dau Monk Mode"),
        teloxide::types::BotCommand::new("today", "Tien do hom nay"),
        teloxide::types::BotCommand::new("week", "Tong ket 7 ngay"),
        teloxide::types::BotCommand::new("journal", "Viet journal"),
        teloxide::types::BotCommand::new("plan", "Viet 3 viec quan trong"),
        teloxide::types::BotCommand::new("urge", "Dang nho co ay"),
        teloxide::types::BotCommand::new("help", "Huong dan"),
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
                    bot.send_message(chat_id, format!("✅ Da luu journal cho ngay {date}.")).await?;
                    return Ok(());
                }
                "plan" => {
                    save_priorities(&state.pool, &date, text).await?;
                    mark_task_done(&state.pool, &date, "top3").await?;
                    clear_session(&state.pool, state.owner_id).await?;
                    bot.send_message(chat_id, format!("✅ Da luu 3 viec quan trong cho ngay {date}.")).await?;
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
            bot.send_message(chat_id, "Lenh khong ro. Go /help de xem huong dan.").await?;
        }
    }

    Ok(())
}

async fn handle_callback(bot: Bot, q: CallbackQuery, state: Arc<AppState>) -> HandlerResult {
    if q.from.id.0 as i64 != state.owner_id {
        bot.answer_callback_query(q.id).text("Khong co quyen.").await?;
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
            bot.answer_callback_query(q.id).text("Callback loi.").await?;
            return Ok(());
        }
        let date = parts[0];
        let section_id = parts[1];
        let task_id = parts[2];
        toggle_task(&state.pool, date, task_id).await?;
        bot.answer_callback_query(q.id).text("Da cap nhat.").await?;
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
        bot.answer_callback_query(q.id).text("Da ghi nhan.").await?;
        bot.send_message(chat_id, "Ghi nhan xong. Quay lai duong ray ngay bay gio.").await?;
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
        "🧘 MONK MODE - 90 NGAY\n\nNgay {day}/90 da bat dau.\n\nKhong vi ai khac.\nNhung neu mot ngay co ay nhin lai, co ay se thay mot nguoi dan ong hoan toan khac."
    ))
}

fn help_text() -> &'static str {
    "Lenh:\n/start - bat dau\n/today - tien do hom nay\n/week - tong ket tuan\n/plan - ghi 3 viec quan trong\n/journal - ghi journal\n/urge - dang nho co ay\n\nBot chi tra loi owner Telegram ID trong .env."
}

fn section_text(date: &str, section: &SectionDef) -> String {
    format!("{}\nNgay: {}\n\n{}\n\nTick khi xong:", section.title, date, section.intro)
}

fn journal_prompt(date: &str) -> String {
    format!(
        "📓 Journal ngay {date}\n\nTra loi mot tin nhan voi 5 dong:\n1. Hom nay minh lam tot gi?\n2. Sai gi?\n3. Hoc duoc gi?\n4. Biet on dieu gi?\n5. Mai lam gi?"
    )
}

fn plan_prompt(date: &str) -> String {
    format!("📒 Ke hoach ngay {date}\n\nGui 3 viec quan trong nhat hom nay, moi viec mot dong.")
}

fn urge_text() -> &'static str {
    "🚨 DANG NHO CO AY\n\nDung lai 10 phut.\n\nKhong mo story.\nKhong doc tin cu.\nKhong nhan khi dang nho.\n\nLam 1 trong 3:\n🏋️ Gym\n🚶 Di bo\n📓 Viet 5 dong\n\nChon ket qua ben duoi."
}

fn main_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("📋 Hom nay", "today"),
            InlineKeyboardButton::callback("📊 Tuan nay", "week"),
        ],
        vec![
            InlineKeyboardButton::callback("📒 Ke hoach", "plan"),
            InlineKeyboardButton::callback("📓 Journal", "journal"),
        ],
        vec![InlineKeyboardButton::callback("🚨 Dang nho co ay", "urge")],
    ])
}

fn today_keyboard(_date: &str) -> InlineKeyboardMarkup {
    main_menu_keyboard()
}

fn urge_keyboard(date: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("✅ Toi da vuot qua", format!("urge|pass|{date}"))],
        vec![InlineKeyboardButton::callback("❌ Toi da stalk", format!("urge|stalk|{date}"))],
    ])
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
    rows.push(vec![InlineKeyboardButton::callback("📋 Xem tien do hom nay", "today")]);
    rows.push(vec![InlineKeyboardButton::callback("🚨 Dang nho co ay", "urge")]);
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
            "INSERT OR IGNORE INTO monk_tasks (date, task_id, section_id, title, points, completed)
             VALUES (?, ?, ?, ?, ?, 0)",
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
        format!("🌙 Tong ket ngay {day}/90"),
        format!("Ngay: {date}"),
        String::new(),
        format!("Diem: {score}/100"),
        format!("Hoan thanh: {done_count}/{}", rows.len()),
        String::new(),
    ];

    for row in rows {
        let icon = if row.get::<i64, _>("completed") != 0 { "✅" } else { "❌" };
        lines.push(format!("{icon} {}", row.get::<String, _>("title")));
    }

    if let Some(priorities) = priorities {
        lines.push(String::new());
        lines.push(format!("📒 3 viec:\n{priorities}"));
    }
    if journal.is_some() {
        lines.push(String::new());
        lines.push("📓 Journal: da luu".to_string());
    }

    let comment = if score >= 85 {
        "Nhan xet: Hom nay rat tot. Ky luat dang thang cam xuc."
    } else if score >= 60 {
        "Nhan xet: Chua hoan hao, nhung ban van dang o tren duong ray."
    } else {
        "Nhan xet: Hom nay yeu. Dung tu danh minh, ngay mai quay lai nhip."
    };
    lines.push(String::new());
    lines.push(comment.to_string());
    lines.push(format!("Gio tong ket hang ngay: {}", state.daily_summary_time));

    Ok(lines.join("\n"))
}

async fn weekly_summary_text(pool: &SqlitePool, date: &str) -> Result<String> {
    let end = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
    let start = end - Duration::days(6);
    let mut lines = vec![
        "📊 Monk Mode - Tong ket 7 ngay".to_string(),
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

    lines.push(String::new());
    lines.push(format!("Diem trung binh: {avg}/100"));
    for row in urge_rows {
        lines.push(format!("{}: {}", row.get::<String, _>("kind"), row.get::<i64, _>("count")));
    }
    lines.push(String::new());
    lines.push("Ket luan: Dung can hoan hao. Can quay lai moi ngay.".to_string());
    Ok(lines.join("\n"))
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
