use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::feed::{FeedItem, SourceKind};

const NOTE_COLOR: u32 = 0x41c9b4;
const YOUTUBE_COLOR: u32 = 0xff0000;

pub struct NotifyTarget<'a> {
    pub webhook_url: &'a str,
    pub username: Option<&'a str>,
    pub mention: Option<&'a str>,
}

#[allow(async_fn_in_trait)]
pub trait Notifier {
    async fn notify(&self, target: &NotifyTarget<'_>, item: &FeedItem) -> Result<()>;
}

pub struct DiscordWebhookNotifier {
    http: reqwest::Client,
}

impl DiscordWebhookNotifier {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("Hakuren-Bot/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to build webhook http client")?;
        Ok(Self { http })
    }
}

impl Notifier for DiscordWebhookNotifier {
    async fn notify(&self, target: &NotifyTarget<'_>, item: &FeedItem) -> Result<()> {
        if target.webhook_url.is_empty() {
            anyhow::bail!("empty webhook url");
        }

        let mut payload = json!({ "embeds": [build_embed(item)] });
        if let Some(name) = target.username {
            payload["username"] = json!(name);
        }
        if let Some(m) = target.mention {
            payload["content"] = json!(m);
        }

        for attempt in 1..=3 {
            let resp = self
                .http
                .post(target.webhook_url)
                .json(&payload)
                .send()
                .await
                .context("webhook request failed")?;

            let status = resp.status();
            if status.is_success() {
                return Ok(());
            }

            if status.as_u16() == 429 && attempt < 3 {
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0)
                    .max(0.5);
                tracing::warn!("discord rate limit, retry in {wait:.1}s");
                tokio::time::sleep(Duration::from_secs_f64(wait)).await;
                continue;
            }

            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "webhook returned {status}: {}",
                body.chars().take(200).collect::<String>()
            );
        }

        anyhow::bail!("webhook failed after retries")
    }
}

fn build_embed(item: &FeedItem) -> Value {
    let (color, footer) = match item.source {
        SourceKind::Note => (NOTE_COLOR, "note"),
        SourceKind::Youtube => (YOUTUBE_COLOR, "YouTube"),
    };

    let mut author = json!({ "name": item.author_name });
    if let Some(url) = item.author_url.as_deref().filter(|s| !s.is_empty()) {
        author["url"] = json!(url);
    }
    if let Some(icon) = item.author_icon.as_deref().filter(|s| !s.is_empty()) {
        author["icon_url"] = json!(icon);
    }

    let mut embed = json!({
        "title": truncate(&item.title, 256),
        "url": item.url,
        "color": color,
        "author": author,
        "footer": { "text": footer },
    });

    if let Some(desc) = item.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        embed["description"] = json!(truncate(desc, 400));
    }
    if let Some(img) = item.thumbnail.as_deref().filter(|s| !s.is_empty()) {
        embed["image"] = json!({ "url": img });
    }
    if let Some(dt) = item.published_at {
        embed["timestamp"] = json!(dt.to_rfc3339());
    }
    if !item.fields.is_empty() {
        embed["fields"] = Value::Array(
            item.fields
                .iter()
                .map(|f| json!({ "name": f.name, "value": f.value, "inline": true }))
                .collect(),
        );
    }

    embed
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
