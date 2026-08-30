use std::time::Duration;

use teloxide::prelude::*;

/// Touched only after a successful getMe — the Bot API's ping — so its mtime
/// is the age of the last confirmed connection to Telegram. The container
/// health check in the Dockerfile reads exactly this.
const HEARTBEAT: &str = "/tmp/health";

pub fn start(bot: Bot) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            match bot.get_me().await {
                Ok(_) => {
                    if let Err(err) = std::fs::File::create(HEARTBEAT) {
                        log::error!("Failed to write heartbeat: {err:?}");
                    }
                }
                Err(err) => log::warn!("Connection check failed: {err:?}"),
            }
        }
    });
}
