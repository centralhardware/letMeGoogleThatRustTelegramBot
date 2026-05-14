use std::sync::Arc;
use std::time::Instant;

use teloxide::{
    payloads::AnswerInlineQuery,
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputMessageContentText, LinkPreviewOptions, ParseMode
    },
};
use urlencoding::encode;

mod logging;

use crate::logging::{Logger, UpdateContext};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("starting up");

    let bot = Bot::from_env();
    let logger = Logger::from_env().map(Arc::new);
    if logger.is_some() {
        log::info!("ClickHouse logging enabled");
    } else {
        log::info!("ClickHouse logging disabled (CLICKHOUSE_URL not set)");
    }

    let handler = Update::filter_inline_query().branch(dptree::endpoint(
        move |bot: Bot, q: InlineQuery| {
            let logger = logger.clone();
            async move {
                log::info!("Received inline query: {:?} from {:?} {:?} {:?} {:?}", &q.query, &q.from.id, &q.from.first_name, &q.from.last_name, &q.from.username);

                let ctx = build_context(logger.as_deref(), &q.from).await;
                if let Some(l) = logger.as_deref() {
                    let in_payload = serde_json::to_string(&q).unwrap_or_default();
                    l.write(&ctx, "InlineQueryUpdate", &q.id.0, &in_payload, true, "", 0).await;
                }

                let results = if q.query.is_empty() {
                    vec![
                        get_article("1".to_string(), "shrugs".to_string(), "¯\\_(ツ)_/¯".to_string(), "".to_string()),
                        get_article("2".to_string(), "nometa".to_string(), "nometa.xyz".to_string(), "".to_string()),
                        get_article("3".to_string(), "How do I ask a good question?".to_string(), "https://stackoverflow.com/help/how-to-ask".to_string(), "".to_string()),
                        get_article("4".to_string(), "use pastebin".to_string(), "Use pastebin.com or gist.github.com to share code or long text.".to_string(), "".to_string())
                    ]
                } else {
                    get_articles(q.query.clone())
                };

                let payload = AnswerInlineQuery::new(q.id.clone(), results);
                let request_json = serde_json::to_string(&payload).unwrap_or_default();
                let started = Instant::now();
                let response = bot.answer_inline_query(payload.inline_query_id.clone(), payload.results.clone()).send().await;
                let duration_ms = started.elapsed().as_millis() as u32;

                let (success, response_str, error_str) = match &response {
                    Ok(_) => (true, "true".to_string(), String::new()),
                    Err(err) => (false, String::new(), format!("{err:?}")),
                };

                if let Some(l) = logger.as_deref() {
                    l.write(&ctx, "answerInlineQuery", &request_json, &response_str, success, &error_str, duration_ms).await;
                }

                if let Err(err) = &response {
                    log::error!("Error in handler: {err:?}");
                }
                respond(())
            }
        }
    ));

    Dispatcher::builder(bot, handler).enable_ctrlc_handler().build().dispatch().await;
}

async fn build_context(logger: Option<&Logger>, user: &teloxide::types::User) -> UpdateContext {
    let update_id = match logger {
        Some(l) => l.next_update_id().await,
        None => 0,
    };
    UpdateContext::from_user(update_id, user)
}

fn get_articles(query: String) -> Vec<InlineQueryResult> {
    let services = vec![
        ("letmegooglethat.com", "https://letmegooglethat.com/?q="),
        ("googlethatforyou.com", "https://googlethatforyou.com?q="),
        ("lmgtfy.app", "https://lmgtfy.app/?q="),
        ("www.google.com", "https://www.google.com/search?q="),
        ("stackoverflow.com", "https://stackoverflow.com/search?q=")
    ];
    services.iter()
        .enumerate()
        .map(|(i, (name, url))|
            get_article(i.to_string(),
                        name.to_string(),
                        get_html_link(&format!("{}{}", url, encode(&query)), &query),
                        get_icon_url(name))
        )
        .collect()
}

fn get_article(id: String, title: String, content: String, thumb_url: String) -> InlineQueryResult {
    let mut article = InlineQueryResultArticle::new(
        id,
        title,
        InputMessageContent::Text(InputMessageContentText::new(
            content
        ).parse_mode(ParseMode::Html).link_preview_options(LinkPreviewOptions {
            is_disabled: true,
            url: None,
            prefer_small_media: false,
            prefer_large_media: false,
            show_above_text: false,
        })),
    );
    if !thumb_url.is_empty() {
        article = article.thumbnail_url(thumb_url.parse().unwrap());
    }
    InlineQueryResult::Article(article)
}

fn get_html_link(link: &str, title: &str) -> String {
    format!("<a href=\"{}\">{}</a>", link, title)
}

fn get_icon_url(url: &str) -> String {
    format!("https://www.google.com/s2/favicons?sz=64&domain_url={}", url)
}
