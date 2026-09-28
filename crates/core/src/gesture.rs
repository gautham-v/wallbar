//! Turning scroll events into single steps.
//!
//! A trackpad swipe is dozens of scroll events, followed by momentum events
//! that keep coming after the fingers lift. One swipe should be one step, so
//! a [`StepGate`] adds the deltas up until they pass a threshold, fires once,
//! and then stays shut until the next gesture begins (or, for devices that
//! never say a gesture began, until the events have stopped for a moment).
//! A notched mouse wheel has no gestures: each notch is a step, with a short
//! cooldown so a flick is not ten.

use std::time::{Duration, Instant};

/// Where a scroll event sits in its gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Fingers went down: a new gesture.
    Began,
    /// Anything else: movement, the end, momentum.
    Changed,
}

/// Which way to step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Next,
    Prev,
}

/// Precise deltas (trackpad, Magic Mouse) must add up to this many points.
pub const THRESHOLD: f32 = 40.0;
/// After firing, a gesture that never reports `Began` reopens after the
/// events have been quiet this long.
pub const QUIET: Duration = Duration::from_millis(350);
/// A notched wheel steps at most this often.
pub const WHEEL_COOLDOWN: Duration = Duration::from_millis(220);

#[derive(Debug, Default)]
pub struct StepGate {
    acc: f32,
    fired: bool,
    last_event: Option<Instant>,
    last_step: Option<Instant>,
}

impl StepGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one event. `delta` is already signed so that negative means
    /// "next" (see the callers for the direction conventions); `precise` is
    /// whether it came from a trackpad-like device.
    pub fn feed(&mut self, delta: f32, phase: Phase, precise: bool, now: Instant) -> Option<Step> {
        let quiet = self
            .last_event
            .is_none_or(|t| now.duration_since(t) >= QUIET);
        self.last_event = Some(now);

        if !precise {
            if delta == 0.0 {
                return None;
            }
            if self
                .last_step
                .is_some_and(|t| now.duration_since(t) < WHEEL_COOLDOWN)
            {
                return None;
            }
            self.last_step = Some(now);
            return Some(direction(delta));
        }

        if phase == Phase::Began || quiet {
            self.acc = 0.0;
            self.fired = false;
        }
        if self.fired {
            return None;
        }
        // A change of direction mid-gesture starts the count again.
        if self.acc != 0.0 && delta.signum() != self.acc.signum() {
            self.acc = 0.0;
        }
        self.acc += delta;
        if self.acc.abs() >= THRESHOLD {
            self.fired = true;
            self.last_step = Some(now);
            let step = direction(self.acc);
            self.acc = 0.0;
            return Some(step);
        }
        None
    }
}

fn direction(delta: f32) -> Step {
    if delta < 0.0 {
        Step::Next
    } else {
        Step::Prev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn one_swipe_is_one_step_however_long_it_runs() {
        let t = Instant::now();
        let mut g = StepGate::new();
        let mut steps = Vec::new();
        steps.extend(g.feed(-5.0, Phase::Began, true, at(t, 0)));
        for i in 1..60 {
            // Movement, then momentum: all part of the same swipe.
            steps.extend(g.feed(-12.0, Phase::Changed, true, at(t, i * 10)));
        }
        assert_eq!(steps, vec![Step::Next]);
        // The next swipe is another step.
        assert_eq!(g.feed(20.0, Phase::Began, true, at(t, 700)), None);
        assert_eq!(
            g.feed(25.0, Phase::Changed, true, at(t, 710)),
            Some(Step::Prev)
        );
    }

    #[test]
    fn small_movements_do_not_step() {
        let t = Instant::now();
        let mut g = StepGate::new();
        assert_eq!(g.feed(-10.0, Phase::Began, true, at(t, 0)), None);
        assert_eq!(g.feed(-10.0, Phase::Changed, true, at(t, 10)), None);
        // Wobbling back resets the count rather than adding across directions.
        assert_eq!(g.feed(15.0, Phase::Changed, true, at(t, 20)), None);
        assert_eq!(g.feed(-30.0, Phase::Changed, true, at(t, 30)), None);
    }

    #[test]
    fn a_gesture_without_a_beginning_reopens_after_a_pause() {
        let t = Instant::now();
        let mut g = StepGate::new();
        assert_eq!(
            g.feed(-50.0, Phase::Changed, true, at(t, 0)),
            Some(Step::Next)
        );
        assert_eq!(g.feed(-50.0, Phase::Changed, true, at(t, 100)), None);
        assert_eq!(
            g.feed(-50.0, Phase::Changed, true, at(t, 1000)),
            Some(Step::Next)
        );
    }

    #[test]
    fn a_wheel_steps_per_notch_with_a_cooldown() {
        let t = Instant::now();
        let mut g = StepGate::new();
        assert_eq!(
            g.feed(-1.0, Phase::Changed, false, at(t, 0)),
            Some(Step::Next)
        );
        assert_eq!(g.feed(-1.0, Phase::Changed, false, at(t, 50)), None);
        assert_eq!(
            g.feed(1.0, Phase::Changed, false, at(t, 300)),
            Some(Step::Prev)
        );
        assert_eq!(g.feed(0.0, Phase::Changed, false, at(t, 900)), None);
    }
}
