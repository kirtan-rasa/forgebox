use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::Sender, Arc};

#[derive(Clone)]
pub struct ExecutionControl {
    cancelled: Arc<AtomicBool>,
    status_tx: Sender<String>,
}

impl ExecutionControl {
    pub fn new(status_tx: Sender<String>) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            status_tx,
        }
    }

    pub(crate) fn silent() -> Self {
        let (status_tx, _status_rx) = std::sync::mpsc::channel();
        Self::new(status_tx)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.report_status("Stopping current process…");
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn report_status(&self, status: impl Into<String>) {
        let _ = self.status_tx.send(status.into());
    }
}

#[cfg(test)]
mod tests {
    use super::ExecutionControl;

    #[test]
    fn cancellation_state_is_shared_between_clones() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let control = ExecutionControl::new(tx);
        let worker = control.clone();

        assert!(!worker.is_cancelled());
        control.cancel();
        assert!(worker.is_cancelled());
    }
}
