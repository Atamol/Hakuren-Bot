use std::path::Path;
use std::time::Duration;

use anyhow::Result;

use crate::config::{Config, Watch, YoutubeWatch};
use crate::feed::FeedItem;
use crate::note::NoteClient;
use crate::notifier::{DiscordWebhookNotifier, Notifier, NotifyTarget};
use crate::state::State;
use crate::youtube::YoutubeClient;

pub async fn run(config: Config) -> Result<()> {
    let note = NoteClient::new()?;
    let youtube = YoutubeClient::new()?;
    let notifier = DiscordWebhookNotifier::new()?;
    let mut state = State::load(Path::new(&config.state_path))?;

    tracing::info!(
        "watching {} note + {} youtube source(s), polling every {}s",
        config.watch.len(),
        config.youtube.len(),
        config.poll_interval_secs
    );

    loop {
        for w in &config.watch {
            if let Err(e) = poll_note(&config, &note, &notifier, &mut state, w).await {
                tracing::warn!("note poll failed for '{}': {e:#}", w.urlname);
            }
        }
        for y in &config.youtube {
            if let Err(e) = poll_youtube(&config, &youtube, &notifier, &mut state, y).await {
                tracing::warn!("youtube poll failed for '{}': {e:#}", y.channel);
            }
        }

        if let Err(e) = state.save() {
            tracing::warn!("failed to save state: {e:#}");
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(config.poll_interval_secs)) => {}
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutdown requested, saving state");
                state.save().ok();
                return Ok(());
            }
        }
    }
}

async fn poll_note<N: Notifier>(
    config: &Config,
    note: &NoteClient,
    notifier: &N,
    state: &mut State,
    w: &Watch,
) -> Result<()> {
    let items: Vec<FeedItem> = note
        .latest_notes(&w.urlname)
        .await?
        .into_iter()
        .filter(|n| n.is_published())
        .map(|n| n.into_item())
        .collect();

    let target = NotifyTarget {
        webhook_url: config.webhook_for(&w.webhook_url),
        username: config.username_for(&w.discord_username),
        mention: w.mention.as_deref(),
    };
    process(config, notifier, state, &w.urlname, &w.urlname, items, &target).await
}

async fn poll_youtube<N: Notifier>(
    config: &Config,
    youtube: &YoutubeClient,
    notifier: &N,
    state: &mut State,
    y: &YoutubeWatch,
) -> Result<()> {
    let channel_id = youtube.resolve_channel_id(&y.channel).await?;
    let items = youtube.latest_videos(&channel_id).await?;
    let state_key = format!("youtube:{channel_id}");

    let target = NotifyTarget {
        webhook_url: config.webhook_for(&y.webhook_url),
        username: config.username_for(&y.discord_username),
        mention: y.mention.as_deref(),
    };
    process(config, notifier, state, &state_key, &y.channel, items, &target).await
}

async fn process<N: Notifier>(
    config: &Config,
    notifier: &N,
    state: &mut State,
    state_key: &str,
    label: &str,
    mut items: Vec<FeedItem>,
    target: &NotifyTarget<'_>,
) -> Result<()> {
    items.sort_by_key(|i| i.published_at);

    let cs = state.creator_mut(state_key);

    // 初回は既存分を記録するだけで通知しない
    if !cs.initialized {
        cs.initialized = true;
        for it in &items {
            cs.seen_keys.insert(it.id.clone());
        }
        tracing::info!("'{}' initialized with {} existing item(s)", label, items.len());

        if config.notify_on_first_run {
            for it in &items {
                if let Err(e) = notifier.notify(target, it).await {
                    tracing::warn!("first-run notify failed for '{}' ({}): {e:#}", label, it.id);
                }
            }
        }
        return Ok(());
    }

    for it in &items {
        if cs.seen_keys.contains(&it.id) {
            continue;
        }
        match notifier.notify(target, it).await {
            Ok(()) => {
                cs.seen_keys.insert(it.id.clone());
                tracing::info!("notified '{}': {}", label, it.title);
            }
            Err(e) => {
                tracing::warn!("notify failed for '{}' ({}): {e:#}", label, it.id);
            }
        }
    }

    Ok(())
}
