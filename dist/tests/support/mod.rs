use std::{collections::VecDeque, sync::Mutex};
use watane_dist::{Confirm, Decision, SignRequest};
#[derive(Default)]
pub struct RecordingConfirm {
    pub decisions: Mutex<VecDeque<Decision>>,
    pub requests: Mutex<Vec<SignRequest>>,
}
impl RecordingConfirm {
    pub fn new(decisions: impl IntoIterator<Item = Decision>) -> Self {
        Self {
            decisions: Mutex::new(decisions.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
}
impl Confirm for RecordingConfirm {
    fn confirm(&self, request: &SignRequest) -> Decision {
        self.requests.lock().unwrap().push(request.clone());
        self.decisions
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected confirmation")
    }
}
