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
    pub fn send(&self, message: Message) -> Result<(), mpsc::SendError<()>> {
        // Shutdown is the only send failure; these UI messages are never retried.
        self.tx.send(message).map_err(|_| mpsc::SendError(()))?;
        self.ctx.request_repaint();
        Ok(())
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
        std::thread::spawn(move || worker.send(Message::StateSaved(Ok(()))).unwrap())
            .join()
            .unwrap();
        assert!(*received.lock().unwrap());
    }
}
