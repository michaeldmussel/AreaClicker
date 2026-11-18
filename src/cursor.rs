//! Cursor movement and clicking.
//!
//! Moves the cursor in a straight line at a constant speed, then clicks.
//! Every wait is interruptible, so stopping or pausing a run takes effect
//! within one sleep chunk (10 ms).

use enigo::{MouseButton, MouseControllable};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{thread, time::Duration};

/// Granularity of interruptible sleeps.
const SLEEP_CHUNK_MS: u64 = 10;

#[derive(Clone, Debug)]
pub struct MoveSettings {
    /// Cursor speed in pixels per second.
    pub speed_px_per_sec: f32,
    /// Time between two cursor updates.
    pub step_ms: u64,
    /// How long the button is held down for a click.
    pub hold_ms: u64,
}

impl Default for MoveSettings {
    fn default() -> Self {
        Self {
            speed_px_per_sec: 1500.0,
            step_ms: 10,
            hold_ms: 40,
        }
    }
}

/// Number of cursor updates needed to cover the distance at the configured speed.
/// Always at least 1, so the cursor always ends exactly on the target.
pub fn step_count(from: (i32, i32), to: (i32, i32), settings: &MoveSettings) -> usize {
    let dx = (to.0 - from.0) as f32;
    let dy = (to.1 - from.1) as f32;
    let distance = dx.hypot(dy);
    let px_per_step = settings.speed_px_per_sec * settings.step_ms.max(1) as f32 / 1000.0;
    if px_per_step <= 0.0 {
        return 1;
    }
    ((distance / px_per_step).ceil() as usize).max(1)
}

/// Evenly spaced points on the straight line from `from` to `to`.
/// Excludes the start point and always ends exactly on `to`.
pub fn linear_path(
    from: (i32, i32),
    to: (i32, i32),
    steps: usize,
) -> impl Iterator<Item = (i32, i32)> {
    let steps = steps.max(1);
    (1..=steps).map(move |i| {
        let t = i as f32 / steps as f32;
        let x = from.0 as f32 + (to.0 - from.0) as f32 * t;
        let y = from.1 as f32 + (to.1 - from.1) as f32 * t;
        (x.round() as i32, y.round() as i32)
    })
}

/// Sleeps for `duration_ms`, waiting while `paused` is set.
/// Returns `false` as soon as `running` is cleared.
pub fn interruptible_sleep(
    duration_ms: u64,
    running: &Arc<AtomicBool>,
    paused: &Arc<AtomicBool>,
) -> bool {
    let mut remaining = duration_ms;
    while remaining > 0 {
        if !running.load(Ordering::Relaxed) {
            return false;
        }
        while paused.load(Ordering::Relaxed) {
            if !running.load(Ordering::Relaxed) {
                return false;
            }
            thread::sleep(Duration::from_millis(SLEEP_CHUNK_MS));
        }
        let chunk = remaining.min(SLEEP_CHUNK_MS);
        thread::sleep(Duration::from_millis(chunk));
        remaining -= chunk;
    }
    running.load(Ordering::Relaxed)
}

/// Moves the cursor in a straight line to `to`, then clicks `button`.
/// Returns `false` if the run was stopped before the click completed.
/// The button is always released, even when stopped mid-click.
pub fn move_and_click(
    mouse: &mut impl MouseControllable,
    from: (i32, i32),
    to: (i32, i32),
    button: MouseButton,
    settings: &MoveSettings,
    running: &Arc<AtomicBool>,
    paused: &Arc<AtomicBool>,
) -> bool {
    let steps = step_count(from, to, settings);
    for (x, y) in linear_path(from, to, steps) {
        if !running.load(Ordering::Relaxed) {
            return false;
        }
        mouse.mouse_move_to(x, y);
        if !interruptible_sleep(settings.step_ms, running, paused) {
            return false;
        }
    }

    mouse.mouse_down(button);
    let completed = interruptible_sleep(settings.hold_ms, running, paused);
    mouse.mouse_up(button);
    completed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records every call instead of moving the real cursor.
    #[derive(Default)]
    struct FakeMouse {
        moves: Vec<(i32, i32)>,
        downs: usize,
        ups: usize,
    }

    impl MouseControllable for FakeMouse {
        fn mouse_move_to(&mut self, x: i32, y: i32) {
            self.moves.push((x, y));
        }
        fn mouse_move_relative(&mut self, _x: i32, _y: i32) {}
        fn mouse_down(&mut self, _button: MouseButton) {
            self.downs += 1;
        }
        fn mouse_up(&mut self, _button: MouseButton) {
            self.ups += 1;
        }
        fn mouse_click(&mut self, _button: MouseButton) {}
        fn mouse_scroll_x(&mut self, _length: i32) {}
        fn mouse_scroll_y(&mut self, _length: i32) {}
        fn main_display_size(&self) -> (i32, i32) {
            (1920, 1080)
        }
        fn mouse_location(&self) -> (i32, i32) {
            self.moves.last().copied().unwrap_or((0, 0))
        }
    }

    fn flags(running: bool) -> (Arc<AtomicBool>, Arc<AtomicBool>) {
        (
            Arc::new(AtomicBool::new(running)),
            Arc::new(AtomicBool::new(false)),
        )
    }

    fn fast() -> MoveSettings {
        MoveSettings {
            speed_px_per_sec: 100_000.0,
            step_ms: 1,
            hold_ms: 1,
        }
    }

    #[test]
    fn path_ends_exactly_on_target() {
        let path: Vec<_> = linear_path((0, 0), (103, -47), 7).collect();
        assert_eq!(path.len(), 7);
        assert_eq!(*path.last().unwrap(), (103, -47));
    }

    #[test]
    fn path_is_monotone_along_both_axes() {
        let path: Vec<_> = linear_path((10, 500), (400, 20), 50).collect();
        assert!(path
            .windows(2)
            .all(|w| w[1].0 >= w[0].0 && w[1].1 <= w[0].1));
    }

    #[test]
    fn zero_steps_still_reaches_target() {
        let path: Vec<_> = linear_path((5, 5), (9, 9), 0).collect();
        assert_eq!(path, vec![(9, 9)]);
    }

    #[test]
    fn step_count_scales_with_distance() {
        let s = MoveSettings::default(); // 15 px per step
        assert_eq!(step_count((0, 0), (0, 0), &s), 1);
        assert_eq!(step_count((0, 0), (150, 0), &s), 10);
        assert_eq!(step_count((0, 0), (151, 0), &s), 11);
    }

    #[test]
    fn move_and_click_moves_then_clicks_once() {
        let (running, paused) = flags(true);
        let mut mouse = FakeMouse::default();
        assert!(move_and_click(
            &mut mouse,
            (0, 0),
            (300, 200),
            MouseButton::Left,
            &fast(),
            &running,
            &paused
        ));
        assert_eq!(*mouse.moves.last().unwrap(), (300, 200));
        assert_eq!((mouse.downs, mouse.ups), (1, 1));
    }

    #[test]
    fn stopped_run_does_not_click() {
        let (running, paused) = flags(false);
        let mut mouse = FakeMouse::default();
        assert!(!move_and_click(
            &mut mouse,
            (0, 0),
            (300, 200),
            MouseButton::Left,
            &fast(),
            &running,
            &paused
        ));
        assert!(mouse.moves.is_empty());
        assert_eq!(mouse.downs, 0);
    }

    #[test]
    fn sleep_returns_false_when_stopped() {
        let (running, paused) = flags(false);
        assert!(!interruptible_sleep(1_000, &running, &paused));
    }
}
