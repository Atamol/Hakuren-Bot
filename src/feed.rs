use chrono::{DateTime, FixedOffset};

#[derive(Clone, Copy)]
pub enum SourceKind {
    Note,
    Youtube,
}

pub struct EmbedField {
    pub name: String,
    pub value: String,
}

pub struct FeedItem {
    pub id: String,
    pub title: String,
    pub url: String,
    pub published_at: Option<DateTime<FixedOffset>>,
    pub author_name: String,
    pub author_url: Option<String>,
    pub author_icon: Option<String>,
    pub description: Option<String>,
    pub thumbnail: Option<String>,
    pub source: SourceKind,
    pub fields: Vec<EmbedField>,
}
