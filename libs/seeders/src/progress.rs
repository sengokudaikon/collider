use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub seeder_name: String,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Update(ProgressUpdate),
    Complete(String),
    Error(String, String),
    Finish,
}

#[derive(Clone)]
pub struct ProgressTracker {
    tx: mpsc::UnboundedSender<ProgressEvent>,
}

impl ProgressTracker {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ProgressEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }

    pub fn update(&self, update: ProgressUpdate) {
        let _ = self.tx.send(ProgressEvent::Update(update));
    }

    pub fn complete(&self, seeder_name: String) {
        let _ = self.tx.send(ProgressEvent::Complete(seeder_name));
    }

    pub fn error(&self, seeder_name: String, error: String) {
        let _ = self.tx.send(ProgressEvent::Error(seeder_name, error));
    }

    pub fn finish(&self) { let _ = self.tx.send(ProgressEvent::Finish); }
}
