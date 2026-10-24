//! Repeated playback against a session clock, with fresh run epochs and cleanup-owned intervals.
use super::{run, Clock, DEADLINE_RANGE};
use crate::input::{Capture, EventKind};
use crate::macro_format::{PlaybackProperties, RepeatMode, MAX_SAFE_INTEGER};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const FIRST_RUN: u64 = 1;
const COUNTER_RANGE: &str =
    "Playback stopped because its run counter exceeded the supported range.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
/// Distinguishes preparation and per-run release from actual input playback without relinquishing session ownership.
pub enum PlaybackStage {
    Starting,
    #[default]
    Running,
    Finishing,
}

#[derive(Clone, Copy)]
/// Immutable repeat settings; finite counts include the first run and intervals are never speed-scaled.
pub struct RepeatPlan {
    pub total: Option<u64>,
    pub interval: Duration,
}
impl Default for RepeatPlan {
    /// Temporary recordings play once without an interval.
    fn default() -> Self {
        Self {
            total: Some(FIRST_RUN),
            interval: Duration::ZERO,
        }
    }
}
impl RepeatPlan {
    /// Copies validated saved properties, ignoring inactive count/interval settings in Once mode.
    pub fn saved(properties: &PlaybackProperties) -> Self {
        let total = match properties.repeat_mode {
            RepeatMode::Once => Some(FIRST_RUN),
            RepeatMode::Fixed => Some(properties.total_runs),
            RepeatMode::Indefinite => None,
        };
        Self {
            total,
            interval: Duration::from_millis(properties.interval_ms),
        }
    }
}

/// Scheduler effects observed at the native input/status boundary; interval deadlines use session-relative time.
pub enum Step<'a> {
    Run(u64),
    Event(&'a EventKind),
    Finish,
    Interval(Duration),
}

/// A run's deadlines remain anchored to its actual start even when event dispatch takes time.
struct RunClock<'a, C> {
    session: &'a C,
    start: Duration,
}
impl<C: Clock> Clock for RunClock<'_, C> {
    /// Returns elapsed run time for the clock contract; scheduling itself always uses absolute session deadlines.
    fn now(&self) -> Duration {
        self.session.now().saturating_sub(self.start)
    }
    /// Translates a recorded offset to an absolute session deadline with checked arithmetic.
    fn wait(&self, at: Duration, cancel: &AtomicBool) -> Result<bool, String> {
        self.session
            .wait(self.start.checked_add(at).ok_or(DEADLINE_RANGE)?, cancel)
    }
}

/// Executes exact finite/indefinite runs, requiring successful cleanup before any full unscaled interval.
/// Cancellation or effect failure prevents every later run; zero-duration loops yield without adding a timing floor.
pub fn repeat(
    capture: &Capture,
    plan: RepeatPlan,
    clock: &impl Clock,
    cancel: &AtomicBool,
    mut effect: impl FnMut(Step<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let mut number = FIRST_RUN;
    while !cancel.load(Ordering::Acquire) {
        effect(Step::Run(number))?;
        let run_clock = RunClock {
            session: clock,
            start: clock.now(),
        };
        run(capture, &run_clock, cancel, |event| {
            effect(Step::Event(event))
        })?;
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        effect(Step::Finish)?;
        if plan.total == Some(number) || cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        wait_interval(plan.interval, clock, cancel, &mut effect)?;
        number = number
            .checked_add(FIRST_RUN)
            .filter(|number| *number <= MAX_SAFE_INTEGER)
            .ok_or(COUNTER_RANGE)?;
        std::thread::yield_now();
    }
    Ok(())
}

/// Waits only between runs, starting the entire interval after successful input release.
fn wait_interval(
    interval: Duration,
    clock: &impl Clock,
    cancel: &AtomicBool,
    effect: &mut impl FnMut(Step<'_>) -> Result<(), String>,
) -> Result<(), String> {
    if interval.is_zero() || cancel.load(Ordering::Acquire) {
        return Ok(());
    }
    let deadline = clock.now().checked_add(interval).ok_or(DEADLINE_RANGE)?;
    effect(Step::Interval(deadline))?;
    clock.wait(deadline, cancel)?;
    Ok(())
}
