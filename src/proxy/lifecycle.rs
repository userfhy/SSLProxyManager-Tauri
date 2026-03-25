pub struct ServerHandle {
    pub handle: tauri::async_runtime::JoinHandle<()>,
    pub shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl ServerHandle {
    pub fn abort(self) {
        let _ = self.shutdown_tx.send(());
        self.handle.abort();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Stopped,
    Starting,
    Running,
    Failed,
}

pub struct ProxyState {
    pub phase: Phase,
    pub generation: u64,
    pub expected: usize,
    pub started: usize,
    pub handles: Vec<ServerHandle>,
}

impl ProxyState {
    pub const fn new() -> Self {
        Self {
            phase: Phase::Stopped,
            generation: 0,
            expected: 0,
            started: 0,
            handles: Vec::new(),
        }
    }
}

pub static PROXY_STATE: parking_lot::Mutex<ProxyState> = parking_lot::Mutex::new(ProxyState::new());
