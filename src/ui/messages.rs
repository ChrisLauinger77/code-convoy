//! Every background UI message wakes the native event loop after enqueueing.
use super::Message;
use eframe::egui;
use std::sync::mpsc;

#[derive(Clone)]
pub(super) struct Sender {
    tx: mpsc::Sender<Message>,
    ctx: egui::Context,
}

pub(super) fn channel(ctx: &egui::Context) -> (Sender, mpsc::Receiver<Message>) {
    let (tx, rx) = mpsc::channel();
    (
        Sender {
            tx,
            ctx: ctx.clone(),
        },
        rx,
    )
}

impl Sender {
    /// Returns false when the UI has closed. Messages are never retried.
    pub fn send(&self, message: Message) -> bool {
        if self.tx.send(message).is_err() {
            return false;
        }
        self.ctx.request_repaint();
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn cloned_worker_sender_enqueues_before_waking_an_idle_ui() {
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let mut output = ctx.run_ui(Default::default(), |_| {});
            output.textures_delta.clear();
        }
        let (tx, rx) = channel(&ctx);
        let received = Arc::new(Mutex::new(false));
        let callback_received = received.clone();
        let rx = Mutex::new(rx);
        ctx.set_request_repaint_callback(move |_| {
            // This also catches accidentally requesting repaint before send.
            let message = rx.lock().unwrap().try_recv().unwrap();
            assert!(matches!(message, Message::StateSaved(Ok(()))));
            *callback_received.lock().unwrap() = true;
        });
        let worker = tx.clone();
        std::thread::spawn(move || assert!(worker.send(Message::StateSaved(Ok(())))))
            .join()
            .unwrap();
        assert!(*received.lock().unwrap());
        // Replacing the callback drops its receiver; failed sends must not wake.
        ctx.set_request_repaint_callback(|_| panic!("Closed UI must not be woken"));
        assert!(!tx.send(Message::StateSaved(Ok(()))));
    }
}
