use chrono::{DateTime, Utc};
use clickhouse::{Client, Row};
use serde::Serialize;
use teloxide::types::User;

#[derive(Row, Serialize)]
struct BotRequest<'a> {
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    timestamp: DateTime<Utc>,
    bot: &'a str,
    update_id: i64,
    user_id: i64,
    username: &'a str,
    first_name: &'a str,
    last_name: &'a str,
    method: &'a str,
    request: &'a str,
    response: &'a str,
    success: bool,
    error: &'a str,
    duration_ms: u32,
}

#[derive(Clone)]
pub struct UpdateContext {
    pub update_id: i64,
    pub user_id: i64,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
}

impl UpdateContext {
    pub fn from_user(update_id: i64, user: &User) -> Self {
        Self {
            update_id,
            user_id: user.id.0 as i64,
            username: user.username.clone().unwrap_or_default(),
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone().unwrap_or_default(),
        }
    }
}

#[derive(Clone)]
pub struct Logger {
    client: Client,
    bot_name: String,
}

impl Logger {
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("CLICKHOUSE_URL").ok()?;
        let bot_name = std::env::var("BOT_NAME").unwrap_or_else(|_| "letmegooglethatbot".to_string());

        let mut client = Client::default().with_url(url);
        if let Ok(u) = std::env::var("CLICKHOUSE_USER") {
            client = client.with_user(u);
        }
        if let Ok(p) = std::env::var("CLICKHOUSE_PASSWORD") {
            client = client.with_password(p);
        }
        client = client.with_database("telegram_bots");

        Some(Self { client, bot_name })
    }

    pub async fn next_update_id(&self) -> i64 {
        match self
            .client
            .query("SELECT generateSnowflakeID()")
            .fetch_one::<u64>()
            .await
        {
            Ok(id) => id as i64,
            Err(e) => {
                log::error!("Failed to fetch next update_id: {e}");
                0
            }
        }
    }

    pub async fn write(
        &self,
        ctx: &UpdateContext,
        method: &str,
        request: &str,
        response: &str,
        success: bool,
        error: &str,
        duration_ms: u32,
    ) {
        let row = BotRequest {
            timestamp: Utc::now(),
            bot: &self.bot_name,
            update_id: ctx.update_id,
            user_id: ctx.user_id,
            username: &ctx.username,
            first_name: &ctx.first_name,
            last_name: &ctx.last_name,
            method,
            request,
            response,
            success,
            error,
            duration_ms,
        };

        let result = async {
            let mut insert = self.client.insert::<BotRequest<'_>>("bot_requests")?;
            insert.write(&row).await?;
            insert.end().await
        }
        .await;

        if let Err(e) = result {
            log::error!("ClickHouse insert failed: {e}");
        }
    }
}
