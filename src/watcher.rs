use std::path::Path;
use std::time::Duration;

use anyhow::Result;

use crate::config::{Config, Watch};
use crate::note::NoteClient;
use crate::notifier::{DiscordWebhookNotifier, Notifier, NotifyTarget};
use crate::state::State;

pub async fn run(config: Config) -> Result<()> {
    let note = NoteClient::new()?;
    let notifier = DiscordWebhookNotifier::new()?;
    let mut state = State::load(Path::new(&config.state_path))?;

    tracing::info!(
        "watching {} creator(s), polling every {}s",
        config.watch.len(),
        config.poll_interval_secs
    );

    loop {
        for w in &config.watch {
            if let Err(e) = poll_one(&config, &note, &notifier, &mut state, w).await {
                tracing::warn!("poll failed for '{}': {e:#}", w.urlname);
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

async fn poll_one<N: Notifier>(
    config: &Config,
    note: &NoteClient,
    notifier: &N,
    state: &mut State,
    w: &Watch,
) -> Result<()> {
    let mut notes = note.latest_notes(&w.urlname).await?;
    notes.retain(|n| n.is_published());
    notes.sort_by_key(|n| n.publish_at);

    let target = NotifyTarget {
        webhook_url: config.webhook_for(w),
        username: config.username_for(w),
        mention: w.mention.as_deref(),
    };

    let cs = state.creator_mut(&w.urlname);

    // 初回は既存記事を既読にするだけで通知しない
    if !cs.initialized {
        cs.initialized = true;
        for n in &notes {
            cs.seen_keys.insert(n.key.clone());
        }
        tracing::info!("'{}' initialized with {} existing note(s)", w.urlname, notes.len());

        if config.notify_on_first_run {
            for n in &notes {
                if let Err(e) = notifier.notify_new_note(&target, n).await {
                    tracing::warn!("first-run notify failed for '{}' ({}): {e:#}", w.urlname, n.key);
                }
            }
        }
        return Ok(());
    }

    for n in &notes {
        if cs.seen_keys.contains(&n.key) {
            continue;
        }
        match notifier.notify_new_note(&target, n).await {
            Ok(()) => {
                cs.seen_keys.insert(n.key.clone());
                tracing::info!("notified '{}': {}", w.urlname, n.name);
            }
            Err(e) => {
                tracing::warn!("notify failed for '{}' ({}): {e:#}", w.urlname, n.key);
            }
        }
    }

    Ok(())
}
