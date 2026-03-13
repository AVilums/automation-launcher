use std::sync::{Arc, Mutex};

use artifact::ArtifactManifest;

pub enum BgMessage {
    ManifestLoaded(ArtifactManifest),
    Error(String),
    DownloadComplete(String, String),
}

pub struct SharedState {
    pub messages: Vec<BgMessage>,
}

impl SharedState {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            messages: Vec::new(),
        }))
    }
}
