use anyhow::{Context, Result};
use chrono::{DateTime, FixedOffset};
use serde::Deserialize;

const USER_AGENT: &str = concat!("Hakuren-Bot/", env!("CARGO_PKG_VERSION"), " (note watcher)");

#[derive(Debug, Deserialize)]
struct ContentsResponse {
    data: ContentsData,
}

#[derive(Debug, Deserialize)]
struct ContentsData {
    contents: Vec<Note>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Note {
    pub key: String,
    pub name: String,
    #[serde(rename = "noteUrl")]
    pub note_url: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub eyecatch: Option<String>,
    #[serde(rename = "publishAt", default)]
    pub publish_at: Option<DateTime<FixedOffset>>,
    #[serde(rename = "likeCount", default)]
    pub like_count: u64,
    #[serde(rename = "commentCount", default)]
    pub comment_count: u64,
    #[serde(default)]
    pub user: NoteUser,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NoteUser {
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub urlname: String,
    #[serde(rename = "userProfileImagePath", default)]
    pub profile_image: Option<String>,
}

impl Note {
    pub fn is_published(&self) -> bool {
        self.status.is_empty() || self.status == "published"
    }
}

pub struct NoteClient {
    http: reqwest::Client,
}

impl NoteClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .context("failed to build note http client")?;
        Ok(Self { http })
    }

    // ピン留め記事も含まれるので公開日順とは限らない
    pub async fn latest_notes(&self, urlname: &str) -> Result<Vec<Note>> {
        let url =
            format!("https://note.com/api/v2/creators/{urlname}/contents?kind=note&page=1");

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("note API request failed for '{urlname}'"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "note API returned {status} for '{urlname}': {}",
                body.chars().take(200).collect::<String>()
            );
        }

        let parsed: ContentsResponse = resp
            .json()
            .await
            .with_context(|| format!("failed to decode note API json for '{urlname}'"))?;
        Ok(parsed.data.contents)
    }
}
