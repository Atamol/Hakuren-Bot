use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::note::Note;

const NOTE_COLOR: u32 = 0x41c9b4;

pub struct NotifyTarget<'a> {
    pub webhook_url: &'a str,
    pub username: Option<&'a str>,
    pub mention: Option<&'a str>,
}

#[allow(async_fn_in_trait)]
pub trait Notifier {
    async fn notify_new_note(&self, target: &NotifyTarget<'_>, note: &Note) -> Result<()>;
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
    async fn notify_new_note(&self, target: &NotifyTarget<'_>, note: &Note) -> Result<()> {
        if target.webhook_url.is_empty() {
            anyhow::bail!("empty webhook url");
        }

        let mut payload = json!({ "embeds": [build_embed(note)] });
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

fn build_embed(note: &Note) -> Value {
    let author_name = if note.user.nickname.is_empty() {
        note.user.urlname.clone()
    } else {
        note.user.nickname.clone()
    };

    let mut author = json!({ "name": author_name });
    if !note.user.urlname.is_empty() {
        author["url"] = json!(format!("https://note.com/{}", note.user.urlname));
    }
    if let Some(icon) = note.user.profile_image.as_deref().filter(|s| !s.is_empty()) {
        author["icon_url"] = json!(icon);
    }

    let mut embed = json!({
        "title": truncate(&note.name, 256),
        "url": note.note_url,
        "color": NOTE_COLOR,
        "author": author,
        "footer": { "text": "note" },
        "fields": [
            { "name": "スキ", "value": note.like_count.to_string(), "inline": true },
            { "name": "コメント", "value": note.comment_count.to_string(), "inline": true },
        ],
    });

    if let Some(desc) = note.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        embed["description"] = json!(truncate(desc, 400));
    }
    if let Some(img) = note.eyecatch.as_deref().filter(|s| !s.is_empty()) {
        embed["image"] = json!({ "url": img });
    }
    if let Some(dt) = note.publish_at {
        embed["timestamp"] = json!(dt.to_rfc3339());
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
