
use std::time::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TimerState {
    Running,
    Paused,
    Stopped
}

pub struct Timer {
    start: Instant,
    accum: Duration,
    state: TimerState,
}

impl Timer {
    pub fn new(start: bool) -> Self {
        Self {
            start: Instant::now(),
            accum: Duration::ZERO,
            state: if start { TimerState::Running } else { TimerState::Stopped }
        }
    }

    pub fn start(&mut self) {
        self.state = TimerState::Running;
        self.start = Instant::now();
    }

    pub fn pause(&mut self) {
        if self.is_paused() {
            return;
        }

        self.accum = Instant::now().duration_since(self.start);
        self.state = TimerState::Paused;
    }

    pub fn stop(&mut self) {
        if self.is_stopped() {
            return;
        }

        self.accum = Instant::now().duration_since(self.start);
        self.state == TimerState::Stopped;
    }

    pub fn is_stopped(&self) -> bool {
        self.state == TimerState::Stopped
    }

    pub fn is_paused(&self) -> bool {
        self.state >= TimerState::Paused
    }

    pub fn duration(&self) -> Duration {
        self.accum
    }
}