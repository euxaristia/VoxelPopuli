//! Forward sprint controls; hunger and collision eligibility belong to Player.

pub const WALK_SPEED: f32 = 4.317;
pub const SPRINT_SPEED: f32 = 5.612;
pub const SPRINT_JUMP_BOOST: f32 = 4.0; // 0.2 blocks/tick at 20 ticks/second.
pub const SPRINT_EXHAUSTION_PER_BLOCK: f32 = 0.1;
pub const SWIM_EXHAUSTION_PER_BLOCK: f32 = 0.01;
pub const JUMP_EXHAUSTION: f32 = 0.05;
pub const SPRINT_JUMP_EXHAUSTION: f32 = 0.2;
const DOUBLE_TAP_SECONDS: f64 = 0.35;

#[derive(Default)]
pub struct SprintInput {
    forward_down: bool,
    last_forward_press: Option<f64>,
}

impl SprintInput {
    pub fn request(
        &mut self,
        forward_amount: f32,
        sprint_key: bool,
        was_sprinting: bool,
        enabled: bool,
        now: f64,
    ) -> bool {
        if !enabled {
            *self = Self::default();
            return false;
        }
        let forward = forward_amount >= 0.8;
        let mut double_tap = false;
        if forward && !self.forward_down {
            double_tap = self
                .last_forward_press
                .is_some_and(|previous| (0.0..=DOUBLE_TAP_SECONDS).contains(&(now - previous)));
            self.last_forward_press = Some(now);
        }
        self.forward_down = forward;
        forward && (sprint_key || was_sprinting || double_tap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_tap_forward_starts_and_sustains_sprint_until_forward_is_released() {
        let mut input = SprintInput::default();
        assert!(!input.request(1.0, false, false, true, 0.0));
        assert!(!input.request(0.0, false, false, true, 0.1));
        assert!(input.request(1.0, false, false, true, 0.2));
        assert!(input.request(1.0, false, true, true, 2.0));
        assert!(!input.request(0.0, false, true, true, 2.1));
        assert!(!input.request(1.0, false, false, true, 2.2));
    }

    #[test]
    fn sprint_key_needs_forward_input_and_can_be_released_after_starting() {
        let mut input = SprintInput::default();
        for forward in [-1.0, 0.0, 0.5] {
            assert!(!input.request(forward, true, false, true, 0.0));
        }
        assert!(input.request(1.0, true, false, true, 0.0));
        assert!(input.request(1.0, false, true, true, 0.1));
        // Collision or hunger can cancel the actual sprint despite held W.
        assert!(!input.request(1.0, false, false, true, 0.2));
    }

    #[test]
    fn slow_taps_and_input_while_disabled_do_not_start_sprinting() {
        let mut input = SprintInput::default();
        assert!(!input.request(1.0, false, false, true, 0.0));
        assert!(!input.request(0.0, false, false, true, 0.1));
        assert!(!input.request(1.0, false, false, true, 0.5));
        assert!(!input.request(0.0, false, false, false, 0.6));
        assert!(!input.request(1.0, false, false, true, 0.7));
    }
}
