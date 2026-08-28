// Colors and key-framed values shared by every Vibrant Visuals schema.
//
// Any field the docs mark `: optkeyframe` accepts either a bare scalar or an
// object of "time": value stops, where time is a fraction of the day in
// [0, 1]. See the Key Frame JSON Syntax reference.
use super::json::Json;

/// A color as authored in a pack: components in [0, 1], sRGB-encoded.
/// Packs may write an RGB(A) array in 0-255 or a hex string.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Decodes sRGB to linear for use as a light or albedo value. Lighting
    /// math is linear; pack colors are authored in display space.
    pub fn to_linear(self) -> [f32; 3] {
        [
            srgb_to_linear(self.r),
            srgb_to_linear(self.g),
            srgb_to_linear(self.b),
        ]
    }

    /// Raw components with no transfer function, for data channels such as
    /// MERS where each byte is already a linear material parameter.
    pub fn raw(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn parse(value: &Json) -> Option<Color> {
        match value {
            Json::String(text) => parse_hex(text),
            Json::Array(items) => {
                let component =
                    |i: usize| items.get(i)?.as_f32().map(|n| (n / 255.0).clamp(0.0, 1.0));
                match items.len() {
                    3 => Some(Color::rgb(component(0)?, component(1)?, component(2)?)),
                    4 => Some(Color::rgba(
                        component(0)?,
                        component(1)?,
                        component(2)?,
                        component(3)?,
                    )),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

fn parse_hex(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| {
        u8::from_str_radix(digits.get(i * 2..i * 2 + 2)?, 16)
            .ok()
            .map(|v| v as f32 / 255.0)
    };
    match digits.len() {
        6 => Some(Color::rgb(byte(0)?, byte(1)?, byte(2)?)),
        8 => Some(Color::rgba(byte(0)?, byte(1)?, byte(2)?, byte(3)?)),
        _ => None,
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Values that can be interpolated between key frames.
pub trait Lerp: Copy {
    fn lerp(self, other: Self, t: f32) -> Self;
    fn parse(value: &Json) -> Option<Self>;
}

impl Lerp for f32 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }

    fn parse(value: &Json) -> Option<Self> {
        value.as_f32()
    }
}

impl Lerp for Color {
    fn lerp(self, other: Self, t: f32) -> Self {
        Color {
            r: self.r.lerp(other.r, t),
            g: self.g.lerp(other.g, t),
            b: self.b.lerp(other.b, t),
            a: self.a.lerp(other.a, t),
        }
    }

    fn parse(value: &Json) -> Option<Self> {
        Color::parse(value)
    }
}

/// A constant or a set of key frames over the day fraction [0, 1].
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframed<T> {
    // Sorted by time and never empty.
    stops: Vec<(f32, T)>,
}

impl<T: Lerp> Keyframed<T> {
    pub fn constant(value: T) -> Self {
        Self {
            stops: vec![(0.0, value)],
        }
    }

    /// Parses either a scalar (a constant for the whole day) or an object
    /// whose keys are day fractions. Returns None if neither form parses,
    /// so callers can fall back to the documented default.
    pub fn parse(value: &Json) -> Option<Self> {
        if let Some(constant) = T::parse(value) {
            return Some(Self::constant(constant));
        }
        let fields = value.as_object()?;
        let mut stops: Vec<(f32, T)> = fields
            .iter()
            .filter_map(|(key, v)| Some((key.parse::<f32>().ok()?, T::parse(v)?)))
            .collect();
        if stops.is_empty() {
            return None;
        }
        stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        Some(Self { stops })
    }

    /// Samples at a day fraction. Time wraps, and so does the curve: the
    /// last stop interpolates back around to the first across midnight,
    /// which is what keeps dawn continuous for the partial-day curves the
    /// docs use (rayleigh_strength stops at 0.75, for instance).
    pub fn sample(&self, time: f32) -> T {
        let time = time.rem_euclid(1.0);
        let (first_time, first_value) = self.stops[0];
        let (last_time, last_value) = self.stops[self.stops.len() - 1];
        if self.stops.len() == 1 {
            return first_value;
        }
        if time < first_time || time >= last_time {
            // Wrap segment: from the last stop, across 1.0, to the first.
            let span = 1.0 - last_time + first_time;
            if span <= 0.0 {
                return last_value;
            }
            let elapsed = if time >= last_time {
                time - last_time
            } else {
                1.0 - last_time + time
            };
            return last_value.lerp(first_value, elapsed / span);
        }
        let upper = self
            .stops
            .iter()
            .position(|(t, _)| *t > time)
            .unwrap_or(self.stops.len() - 1);
        let (t0, v0) = self.stops[upper - 1];
        let (t1, v1) = self.stops[upper];
        if t1 <= t0 {
            return v0;
        }
        v0.lerp(v1, (time - t0) / (t1 - t0))
    }
}

/// A day-cycle color curve from sRGB 0-255 stops, matching the way pack
/// JSON authors sky and sun colors.
pub fn srgb_curve(stops: &[(f32, [u8; 3])]) -> Keyframed<Color> {
    let object = Json::Object(
        stops
            .iter()
            .map(|(t, rgb)| {
                (
                    t.to_string(),
                    Json::Array(rgb.iter().map(|c| Json::Number(*c as f64)).collect()),
                )
            })
            .collect(),
    );
    Keyframed::parse(&object).expect("srgb curve stops are well formed")
}

/// Reads an optional key-framed field, falling back to a constant default.
pub fn keyframed_or<T: Lerp>(parent: &Json, key: &str, default: T) -> Keyframed<T> {
    parent
        .get(key)
        .and_then(Keyframed::parse)
        .unwrap_or_else(|| Keyframed::constant(default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vibrant::json;

    fn parse_field(source: &str) -> Json {
        json::parse(source).unwrap()
    }

    #[test]
    fn parses_rgb_arrays_and_hex_strings() {
        assert_eq!(
            Color::parse(&parse_field("[255, 0, 127.5]")).unwrap(),
            Color::rgb(1.0, 0.0, 0.5)
        );
        assert_eq!(
            Color::parse(&parse_field(r##""#EFE39D""##)).unwrap(),
            Color::rgb(
                0xEF as f32 / 255.0,
                0xE3 as f32 / 255.0,
                0x9D as f32 / 255.0
            )
        );
        // MERS values are authored as 4-component arrays.
        assert_eq!(
            Color::parse(&parse_field("[0.0, 0.0, 255.0, 0.0]")).unwrap(),
            Color::rgba(0.0, 0.0, 1.0, 0.0)
        );
    }

    #[test]
    fn rejects_malformed_colors() {
        assert_eq!(Color::parse(&parse_field(r##""#ABC""##)), None);
        assert_eq!(Color::parse(&parse_field(r##""#GGGGGG""##)), None);
        assert_eq!(Color::parse(&parse_field("[1, 2]")), None);
        assert_eq!(Color::parse(&parse_field("5")), None);
    }

    #[test]
    fn scalar_field_becomes_a_constant() {
        let curve = Keyframed::<f32>::parse(&parse_field("0.27")).unwrap();
        assert_eq!(curve.sample(0.0), 0.27);
        assert_eq!(curve.sample(0.9), 0.27);
    }

    #[test]
    fn interpolates_between_keyframe_stops() {
        // The sun illuminance curve from the lighting/global.json sample.
        let curve = Keyframed::<f32>::parse(&parse_field(
            r##"{"0.0": 100.0, "0.5": 0.0, "1.0": 100.0}"##,
        ))
        .unwrap();
        assert_eq!(curve.sample(0.0), 100.0);
        assert_eq!(curve.sample(0.25), 50.0);
        assert_eq!(curve.sample(0.5), 0.0);
        assert_eq!(curve.sample(0.75), 50.0);
    }

    #[test]
    fn sorts_out_of_order_stops() {
        let curve =
            Keyframed::<f32>::parse(&parse_field(r##"{"0.5": 0.0, "0.0": 100.0}"##)).unwrap();
        assert_eq!(curve.sample(0.25), 50.0);
    }

    #[test]
    fn wraps_a_partial_day_curve_across_midnight() {
        // rayleigh_strength in the docs runs 0.0 to 0.75 only, so the tail
        // has to close the loop back to the first stop rather than clamp.
        let curve =
            Keyframed::<f32>::parse(&parse_field(r##"{"0.25": 0.0, "0.75": 100.0}"##)).unwrap();
        assert_eq!(curve.sample(0.25), 0.0);
        assert_eq!(curve.sample(0.75), 100.0);
        // Halfway around the wrap segment (0.75 -> 1.0 -> 0.25).
        assert_eq!(curve.sample(0.0), 50.0);
        // And time itself wraps.
        assert_eq!(curve.sample(1.25), 0.0);
        assert_eq!(curve.sample(-0.75), 0.0);
    }

    #[test]
    fn srgb_curve_decodes_255_stops_into_unit_colors() {
        let curve = srgb_curve(&[(0.0, [255, 0, 0]), (1.0, [0, 0, 255])]);
        let noon = curve.sample(0.0);
        assert!((noon.r - 1.0).abs() < 1e-6);
        assert!(noon.g.abs() < 1e-6);
        let mid = curve.sample(0.5);
        assert!((mid.r - 0.5).abs() < 1e-6);
        assert!((mid.b - 0.5).abs() < 1e-6);
    }

    #[test]
    fn interpolates_keyframed_colors() {
        let curve = Keyframed::<Color>::parse(&parse_field(
            r##"{"0.0": [0, 0, 0], "1.0": [255, 255, 255]}"##,
        ))
        .unwrap();
        let mid = curve.sample(0.5);
        assert!((mid.r - 0.5).abs() < 1e-6);
        assert!((mid.b - 0.5).abs() < 1e-6);
    }

    #[test]
    fn falls_back_when_the_field_is_absent_or_junk() {
        let parent = parse_field(r##"{"bad": "not-a-number"}"##);
        assert_eq!(keyframed_or(&parent, "missing", 1.5).sample(0.3), 1.5);
        assert_eq!(keyframed_or(&parent, "bad", 1.5).sample(0.3), 1.5);
    }

    #[test]
    fn srgb_decode_matches_the_standard_curve() {
        assert!(Color::rgb(0.0, 0.0, 0.0).to_linear()[0].abs() < 1e-6);
        assert!((Color::WHITE.to_linear()[0] - 1.0).abs() < 1e-6);
        // 0.5 sRGB is well below 0.5 linear.
        assert!((Color::rgb(0.5, 0.5, 0.5).to_linear()[0] - 0.2140).abs() < 1e-3);
    }
}
