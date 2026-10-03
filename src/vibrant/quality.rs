#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum GraphicsQuality {
    #[cfg_attr(not(target_arch = "wasm32"), default)]
    Cinematic = 0,
    #[cfg_attr(target_arch = "wasm32", default)]
    High = 1,
    Fast = 2,
}

impl GraphicsQuality {
    pub fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Cinematic),
            1 => Some(Self::High),
            2 => Some(Self::Fast),
            _ => None,
        }
    }

    pub fn from_legacy_fancy(fancy: bool) -> Self {
        if fancy { Self::High } else { Self::Fast }
    }

    pub fn is_deferred(self) -> bool {
        self != Self::Fast
    }

    pub fn next(self) -> Self {
        match self {
            Self::Cinematic => Self::High,
            Self::High => Self::Fast,
            Self::Fast => Self::Cinematic,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cinematic => "Cinematic",
            Self::High => "High",
            Self::Fast => "Fast",
        }
    }

    pub fn render_size(self, width: i32, height: i32) -> (i32, i32) {
        let width = width.max(1);
        let height = height.max(1);
        match self {
            Self::Cinematic => (width, height),
            Self::High => (
                ((i64::from(width) * 3) / 4).max(1) as i32,
                ((i64::from(height) * 3) / 4).max(1) as i32,
            ),
            Self::Fast => {
                let aspect = f64::from(width) / f64::from(height);
                let pixel_budget = 1125.0 * 633.0;
                let target_height = (pixel_budget / aspect).sqrt().round().max(1.0);
                let target_width = (target_height * aspect).round().max(1.0);
                (
                    (target_width as i32).min(width),
                    (target_height as i32).min(height),
                )
            }
        }
    }

    pub fn shadow_resolution(self) -> u32 {
        match self {
            Self::Cinematic => 4096,
            Self::High => 2048,
            Self::Fast => 0,
        }
    }

    pub fn effect_steps(self) -> u32 {
        match self {
            Self::Cinematic => 48,
            Self::High => 24,
            Self::Fast => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_cycles_all_presets_without_skipping_high() {
        let mut quality = GraphicsQuality::Cinematic;
        for expected in [
            GraphicsQuality::High,
            GraphicsQuality::Fast,
            GraphicsQuality::Cinematic,
        ] {
            quality = quality.next();
            assert_eq!(quality, expected);
        }
    }

    #[test]
    fn new_settings_use_platform_default_not_legacy_fancy() {
        let expected = if cfg!(target_arch = "wasm32") {
            GraphicsQuality::High
        } else {
            GraphicsQuality::Cinematic
        };
        assert_eq!(GraphicsQuality::default(), expected);
        assert_eq!(
            GraphicsQuality::from_legacy_fancy(true),
            GraphicsQuality::High
        );
        assert_eq!(
            GraphicsQuality::from_legacy_fancy(false),
            GraphicsQuality::Fast
        );
    }

    #[test]
    fn cinematic_uses_native_resolution_high_scales_and_fast_keeps_pixel_budget() {
        assert_eq!(
            GraphicsQuality::Cinematic.render_size(2560, 1440),
            (2560, 1440)
        );
        assert_eq!(GraphicsQuality::High.render_size(2560, 1440), (1920, 1080));
        let (width, height) = GraphicsQuality::Fast.render_size(2560, 1440);
        assert!((width - 1125).abs() <= 1);
        assert!((height - 633).abs() <= 1);
        assert_eq!(GraphicsQuality::Fast.render_size(320, 180), (320, 180));
        assert_eq!(GraphicsQuality::High.render_size(7, 11), (5, 8));
    }

    #[test]
    fn render_sizes_stay_nonzero_and_within_framebuffer_without_integer_overflow() {
        for quality in [
            GraphicsQuality::Cinematic,
            GraphicsQuality::High,
            GraphicsQuality::Fast,
        ] {
            for (width, height) in [
                (0, 0),
                (-20, -1),
                (3840, 0),
                (0, 3840),
                (1, 1),
                (1, i32::MAX),
                (i32::MAX, 1),
                (i32::MAX, i32::MAX),
            ] {
                let size = quality.render_size(width, height);
                assert!((1..=width.max(1)).contains(&size.0));
                assert!((1..=height.max(1)).contains(&size.1));
            }
        }
    }

    #[test]
    fn preset_codes_are_explicit_and_invalid_codes_are_rejected() {
        for (quality, label, resolution, steps) in [
            (GraphicsQuality::Cinematic, "Cinematic", 4096, 48),
            (GraphicsQuality::High, "High", 2048, 24),
            (GraphicsQuality::Fast, "Fast", 0, 0),
        ] {
            assert_eq!(GraphicsQuality::from_byte(quality as u8), Some(quality));
            assert_eq!(quality.label(), label);
            assert_eq!(quality.shadow_resolution(), resolution);
            assert_eq!(quality.effect_steps(), steps);
            assert_eq!(quality.is_deferred(), quality != GraphicsQuality::Fast);
        }
        for invalid in 3..=u8::MAX {
            assert_eq!(GraphicsQuality::from_byte(invalid), None);
        }
    }
}
