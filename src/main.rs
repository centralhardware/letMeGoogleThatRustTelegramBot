use std::sync::Arc;
use std::time::Instant;

use teloxide::utils::html;
use teloxide::{
    payloads::AnswerInlineQuery,
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputMessageContentText,
        LinkPreviewOptions, ParseMode,
    },
};
use urlencoding::encode;

mod health;
mod logging;

use crate::logging::{Logger, UpdateContext};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("starting up");

    let bot = Bot::from_env();
    health::start(bot.clone());
    let logger = Logger::from_env().map(Arc::new);
    if logger.is_some() {
        log::info!("ClickHouse logging enabled");
    } else {
        log::info!("ClickHouse logging disabled (CLICKHOUSE_URL not set)");
    }

    let handler =
        Update::filter_inline_query().branch(dptree::endpoint(move |bot: Bot, q: InlineQuery| {
            let logger = logger.clone();
            async move {
                log::info!(
                    "Received inline query: {:?} from {:?} {:?} {:?} {:?}",
                    q.query,
                    q.from.id,
                    q.from.first_name,
                    q.from.last_name,
                    q.from.username
                );

                let results = if q.query.is_empty() {
                    vec![
                        get_article(
                            "1".to_string(),
                            "shrugs".to_string(),
                            "¯\\_(ツ)_/¯".to_string(),
                            "".to_string(),
                        ),
                        get_article(
                            "2".to_string(),
                            "nometa".to_string(),
                            "nometa.xyz".to_string(),
                            "".to_string(),
                        ),
                        get_article(
                            "3".to_string(),
                            "How do I ask a good question?".to_string(),
                            "https://stackoverflow.com/help/how-to-ask".to_string(),
                            "".to_string(),
                        ),
                        get_article(
                            "4".to_string(),
                            "use pastebin".to_string(),
                            "Use pastebin.com or gist.github.com to share code or long text."
                                .to_string(),
                            "".to_string(),
                        ),
                    ]
                } else {
                    get_articles(q.query.clone())
                };

                // Answer first: inline queries arrive on every keystroke, and logging (which
                // asks ClickHouse for the update id) must never delay the answer.
                let payload = AnswerInlineQuery::new(q.id.clone(), results);
                let request_json = serde_json::to_string(&payload).unwrap_or_default();
                let started = Instant::now();
                let response = bot
                    .answer_inline_query(payload.inline_query_id.clone(), payload.results.clone())
                    .send()
                    .await;
                let duration_ms = started.elapsed().as_millis() as u32;

                let (success, response_str, error_str) = match &response {
                    Ok(_) => (true, "true".to_string(), String::new()),
                    Err(err) => (false, String::new(), format!("{err:?}")),
                };

                if let Some(l) = logger {
                    let in_payload = serde_json::to_string(&q).unwrap_or_default();
                    let query_id = q.id.0.clone();
                    let user = q.from.clone();
                    tokio::spawn(async move {
                        let ctx = UpdateContext::from_user(l.next_update_id().await, &user);
                        l.write(
                            &ctx,
                            "InlineQueryUpdate",
                            &query_id,
                            &in_payload,
                            true,
                            "",
                            0,
                        )
                        .await;
                        l.write(
                            &ctx,
                            "answerInlineQuery",
                            &request_json,
                            &response_str,
                            success,
                            &error_str,
                            duration_ms,
                        )
                        .await;
                    });
                }

                if let Err(err) = &response {
                    log::error!("Error in handler: {err:?}");
                }
                respond(())
            }
        }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

fn get_articles(query: String) -> Vec<InlineQueryResult> {
    let services = [
        ("letmegooglethat.com", "https://letmegooglethat.com/?q="),
        ("googlethatforyou.com", "https://googlethatforyou.com?q="),
        ("lmgtfy.app", "https://lmgtfy.app/?q="),
        ("www.google.com", "https://www.google.com/search?q="),
        ("stackoverflow.com", "https://stackoverflow.com/search?q="),
    ];
    services
        .iter()
        .enumerate()
        .map(|(i, (name, url))| {
            get_article(
                i.to_string(),
                name.to_string(),
                get_html_link(&format!("{}{}", url, encode(&query)), &query),
                get_icon_url(name),
            )
        })
        .collect()
}

fn get_article(id: String, title: String, content: String, thumb_url: String) -> InlineQueryResult {
    let mut article = InlineQueryResultArticle::new(
        id,
        title,
        InputMessageContent::Text(
            InputMessageContentText::new(content)
                .parse_mode(ParseMode::Html)
                .link_preview_options(LinkPreviewOptions {
                    is_disabled: true,
                    url: None,
                    prefer_small_media: false,
                    prefer_large_media: false,
                    show_above_text: false,
                }),
        ),
    );
    match thumb_url.parse() {
        Ok(url) if !thumb_url.is_empty() => article = article.thumbnail_url(url),
        Ok(_) => {}
        Err(err) => log::warn!("Bad thumbnail URL {thumb_url:?}: {err}"),
    }
    InlineQueryResult::Article(article)
}

/// The query is user input: escaped, so `vec<u8>` or `a & b` stays text instead of
/// breaking (or injecting) the message's HTML.
fn get_html_link(link: &str, title: &str) -> String {
    format!(
        "<a href=\"{}\">{}</a>",
        html::escape(link),
        html::escape(title)
    )
}

fn get_icon_url(url: &str) -> String {
    format!(
        "https://www.google.com/s2/favicons?sz=64&domain_url={}",
        url
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(r: &InlineQueryResult) -> String {
        match r {
            InlineQueryResult::Article(a) => match &a.input_message_content {
                InputMessageContent::Text(t) => t.message_text.clone(),
                _ => panic!("not text"),
            },
            _ => panic!("not an article"),
        }
    }

    #[test]
    fn query_is_escaped_and_encoded() {
        let results = get_articles("a<b & c".to_string());
        assert_eq!(results.len(), 5);
        let t = text(&results[3]);
        assert_eq!(
            t,
            "<a href=\"https://www.google.com/search?q=a%3Cb%20%26%20c\">a&lt;b &amp; c</a>"
        );
    }

    #[test]
    fn ids_are_unique() {
        let results = get_articles("x".to_string());
        let ids: std::collections::HashSet<_> = results
            .iter()
            .map(|r| match r {
                InlineQueryResult::Article(a) => a.id.clone(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(ids.len(), 5);
    }

    #[test]
    fn bad_thumbnail_does_not_panic() {
        let r = get_article("1".into(), "t".into(), "c".into(), "not a url".into());
        assert!(matches!(r, InlineQueryResult::Article(ref a) if a.thumbnail_url.is_none()));
        let r = get_article("1".into(), "t".into(), "c".into(), get_icon_url("x.com"));
        assert!(matches!(r, InlineQueryResult::Article(ref a) if a.thumbnail_url.is_some()));
    }
}
