use crate::user_files::{Job, Kind, Outcome};
use std::sync::mpsc;
#[derive(Default)]
pub(super) struct Picker {
    pending: Option<(Job, mpsc::Receiver<Outcome>)>,
}
impl Picker {
    pub(super) fn is_open(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn tick(
        &mut self,
        game: Option<&mut crate::runtime::Game>,
        ctx: &eframe::egui::Context,
    ) {
        let Some(game) = game else {
            self.pending = None;
            return;
        };
        if let Some((job, receiver)) = &self.pending {
            match receiver.try_recv() {
                Ok(outcome) => {
                    game.complete_user_file(job, outcome);
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    game.complete_user_file(job, Outcome::Failed);
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => return,
            }
        }
        let Some(job) = game.take_user_file_job() else {
            return;
        };
        super::window_control::release_pointer(ctx);
        let id = game.id.clone();
        let (sender, receiver) = mpsc::channel();
        let request = job.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new()
                .set_title(format!("GamePKG: {} — {}", id, request.policy.title))
                .add_filter(&request.policy.title, &request.policy.extensions);
            let path = match &request.kind {
                Kind::Import => dialog.pick_file(),
                Kind::Export { name, .. } => dialog.set_file_name(name).save_file(),
            };
            let outcome = crate::user_files::execute(&request, path.as_deref());
            let _ = sender.send(outcome);
            ctx.request_repaint();
        });
        self.pending = Some((job, receiver));
    }
}
