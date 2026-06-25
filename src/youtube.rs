use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::DateTime;
use serde::Deserialize;

use crate::feed::{FeedItem, SourceKind};

const USER_AGENT: &str = "Mozilla/5.0 (compatible; Hakuren-Bot)";

#[derive(Debug, Deserialize)]
struct Feed {
    title: String,
    #[serde(rename = "entry", default)]
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    id: String,
    title: String,
    #[serde(default)]
    published: String,
}

impl Entry {
    fn video_id(&self) -> &str {
        self.id.strip_prefix("yt:video:").unwrap_or(&self.id)
    }
}

pub struct YoutubeClient {
    http: reqwest::Client,
    resolved: Mutex<HashMap<String, String>>,
}

impl YoutubeClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .context("failed to build youtube http client")?;
        Ok(Self {
            http,
            resolved: Mutex::new(HashMap::new()),
        })
    }

    pub async fn resolve_channel_id(&self, input: &str) -> Result<String> {
        if let Some(id) = direct_channel_id(input) {
            return Ok(id);
        }
        if let Some(hit) = self.resolved.lock().unwrap().get(input).cloned() {
            return Ok(hit);
        }
        let id = self.fetch_channel_id(input).await?;
        self.resolved.lock().unwrap().insert(input.to_string(), id.clone());
        Ok(id)
    }

    // RSSはchannel_idしか受け付けないのでハンドルはチャンネルページから解決する
    async fn fetch_channel_id(&self, input: &str) -> Result<String> {
        let url = if input.starts_with("http") {
            input.to_string()
        } else if input.starts_with('@') {
            format!("https://www.youtube.com/{input}")
        } else {
            format!("https://www.youtube.com/@{input}")
        };
        let html = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("failed to load youtube page for '{input}'"))?
            .text()
            .await
            .with_context(|| format!("failed to read youtube page for '{input}'"))?;
        extract_channel_id(&html).with_context(|| format!("could not find channel id for '{input}'"))
    }

    pub async fn latest_videos(&self, channel_id: &str) -> Result<Vec<FeedItem>> {
        let url = format!("https://www.youtube.com/feeds/videos.xml?channel_id={channel_id}");
        let xml = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("youtube feed request failed for '{channel_id}'"))?
            .text()
            .await
            .with_context(|| format!("failed to read youtube feed for '{channel_id}'"))?;

        let feed: Feed = quick_xml::de::from_str(&xml)
            .with_context(|| format!("failed to parse youtube feed for '{channel_id}'"))?;

        let channel_url = format!("https://www.youtube.com/channel/{channel_id}");
        let items = feed
            .entries
            .iter()
            .map(|e| {
                let vid = e.video_id().to_string();
                FeedItem {
                    url: format!("https://www.youtube.com/watch?v={vid}"),
                    thumbnail: Some(format!("https://i.ytimg.com/vi/{vid}/hqdefault.jpg")),
                    id: vid,
                    title: e.title.clone(),
                    published_at: DateTime::parse_from_rfc3339(&e.published).ok(),
                    author_name: feed.title.clone(),
                    author_url: Some(channel_url.clone()),
                    author_icon: None,
                    description: None,
                    source: SourceKind::Youtube,
                    fields: Vec::new(),
                }
            })
            .collect();
        Ok(items)
    }
}

fn direct_channel_id(input: &str) -> Option<String> {
    let s = input
        .strip_prefix("https://www.youtube.com/channel/")
        .or_else(|| input.strip_prefix("channel/"))
        .unwrap_or(input);
    let is_id = s.starts_with("UC")
        && s.len() == 24
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    is_id.then(|| s.to_string())
}

fn extract_channel_id(html: &str) -> Option<String> {
    for marker in [
        "rel=\"canonical\" href=\"https://www.youtube.com/channel/",
        "youtube.com/channel/",
    ] {
        if let Some(pos) = html.find(marker) {
            let rest = &html[pos + marker.len()..];
            let id: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if id.starts_with("UC") && id.len() == 24 {
                return Some(id);
            }
        }
    }
    None
}
