use std::collections::HashMap;

use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use seeders::{ProgressEvent, ProgressUpdate};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct CliProgress {
    multi_progress: MultiProgress,
    progress_bars: HashMap<String, ProgressBar>,
}

impl CliProgress {
    pub fn new() -> Self {
        Self {
            multi_progress: MultiProgress::new(),
            progress_bars: HashMap::new(),
        }
    }

    pub async fn run(
        &mut self, mut progress_rx: mpsc::UnboundedReceiver<ProgressEvent>,
        cancellation_token: CancellationToken,
    ) -> Result<()> {
        loop {
            tokio::select! {
                event = progress_rx.recv() => {
                    match event {
                        Some(ProgressEvent::Update(update)) => {
                            self.handle_update(update).await;
                        }
                        Some(ProgressEvent::Complete(seeder_name)) => {
                            self.handle_complete(seeder_name).await;
                        }
                        Some(ProgressEvent::Error(seeder_name, error)) => {
                            self.handle_error(seeder_name, error).await;
                        }
                        Some(ProgressEvent::Finish) => {
                            break;
                        }
                        None => break,
                    }
                }
                _ = cancellation_token.cancelled() => {
                    self.handle_cancellation().await;
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_update(&mut self, update: ProgressUpdate) {
        let pb = self
            .progress_bars
            .entry(update.seeder_name.clone())
            .or_insert_with(|| {
                let pb = self
                    .multi_progress
                    .add(ProgressBar::new(update.total as u64));
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template(
                            "{prefix:.bold} [{bar:40.cyan/blue}] \
                             {pos:>7}/{len:7} ({percent}%) {msg}",
                        )
                        .unwrap()
                        .progress_chars("=>-"),
                );
                pb.set_prefix(format!("{:12}", update.seeder_name.clone()));
                pb
            });

        pb.set_position(update.current as u64);
        pb.set_length(update.total as u64);
        pb.set_message(update.message.clone());
    }

    async fn handle_complete(&mut self, seeder_name: String) {
        if let Some(pb) = self.progress_bars.get(&seeder_name) {
            pb.finish_with_message("COMPLETE");
        }
    }

    async fn handle_error(&mut self, seeder_name: String, error: String) {
        if let Some(pb) = self.progress_bars.get(&seeder_name) {
            pb.finish_with_message(format!("ERROR: {}", error));
        }
    }

    async fn handle_cancellation(&mut self) {
        for pb in self.progress_bars.values() {
            pb.finish_with_message("CANCELLED");
        }
    }
}
