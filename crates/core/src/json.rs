//! A tiny JSON writer for the HUD and modal view models (no serde at runtime, to keep the Wasm small).
use std::fmt::Write as _;

pub fn write_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if (character as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", character as u32);
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

/// A JavaScript-style number: integers without a fraction, non-finite values as `null`
/// (what `JSON.stringify` writes).
pub fn write_number(out: &mut String, value: f64) {
    if !value.is_finite() {
        out.push_str("null");
    } else if value.fract() == 0.0 && value.abs() < 1e15 {
        let _ = write!(out, "{}", value as i64);
    } else {
        // Only presentation values reach here; nine decimals (trailing zeros trimmed) keep
        // float formatting (and its tables) out of the Wasm.
        let fixed = js_to_fixed(value, 9);
        out.push_str(fixed.trim_end_matches('0').trim_end_matches('.'));
    }
}

/// Writes one JSON object; keys are written in call order.
pub struct ObjectWriter<'a> {
    out: &'a mut String,
    first: bool,
}

impl<'a> ObjectWriter<'a> {
    pub fn new(out: &'a mut String) -> Self {
        out.push('{');
        ObjectWriter { out, first: true }
    }

    fn key(&mut self, key: &str) -> &mut String {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
        write_string(self.out, key);
        self.out.push(':');
        self.out
    }

    pub fn string(&mut self, key: &str, value: &str) -> &mut Self {
        let out = self.key(key);
        write_string(out, value);
        self
    }

    pub fn number(&mut self, key: &str, value: f64) -> &mut Self {
        let out = self.key(key);
        write_number(out, value);
        self
    }

    pub fn integer(&mut self, key: &str, value: i64) -> &mut Self {
        let out = self.key(key);
        let _ = write!(out, "{value}");
        self
    }

    pub fn boolean(&mut self, key: &str, value: bool) -> &mut Self {
        let out = self.key(key);
        out.push_str(if value { "true" } else { "false" });
        self
    }

    /// Writes an already-encoded JSON value.
    pub fn raw(&mut self, key: &str, json: &str) -> &mut Self {
        let out = self.key(key);
        out.push_str(json);
        self
    }

    pub fn finish(self) {
        self.out.push('}');
    }
}

/// Writes `items` as a JSON array using `write_item` for each element.
pub fn write_array<T>(
    out: &mut String,
    items: impl IntoIterator<Item = T>,
    mut write_item: impl FnMut(&mut String, T),
) {
    out.push('[');
    for (index, item) in items.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write_item(out, item);
    }
    out.push(']');
}

/// JavaScript `Number.prototype.toFixed` for the magnitudes the HUD shows: the value is
/// scaled to an integer count of the last digit, with ties rounding away from zero like JS
/// (Rust's formatter rounds them to even). Integer arithmetic keeps float formatting (and its
/// tables) out of the Wasm; values too large to scale exactly fall back to their integer part.
pub fn js_to_fixed(value: f64, digits: usize) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
    }
    let scale = 10u64.pow(digits as u32);
    let magnitude = value.abs();
    let scaled = magnitude * scale as f64;
    // Formatting u64 (not u128) keeps 128-bit division and formatting out of the Wasm.
    let units = if scaled < 9.0e15 {
        round_half_up(magnitude, scale) as u64
    } else {
        (magnitude.trunc() as u64).saturating_mul(scale)
    };
    let mut out = String::with_capacity(24);
    if value < 0.0 {
        out.push('-');
    }
    let _ = write!(out, "{}", units / scale);
    if digits > 0 {
        out.push('.');
        let fraction = (units % scale).to_string();
        for _ in fraction.len()..digits {
            out.push('0');
        }
        out.push_str(&fraction);
    }
    out
}

/// `round(magnitude * scale)` with exact ties rounding up, computed exactly from the value's
/// binary expansion (mantissa times a power of two), so no rounding creeps in before the tie test.
fn round_half_up(magnitude: f64, scale: u64) -> u128 {
    let bits = magnitude.to_bits();
    let biased_exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1 << 52) - 1);
    let (mantissa, exponent) =
        if biased_exponent == 0 { (fraction, -1074) } else { (fraction | (1 << 52), biased_exponent - 1075) };
    let product = u128::from(mantissa) * u128::from(scale);
    if exponent >= 0 {
        return product << exponent;
    }
    let shift = (-exponent) as u32;
    if shift >= 127 {
        return 0;
    }
    let quotient = product >> shift;
    let remainder = product & ((1u128 << shift) - 1);
    if remainder >= 1u128 << (shift - 1) { quotient + 1 } else { quotient }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_fixed_matches_javascript() {
        assert_eq!(js_to_fixed(0.0, 1), "0.0");
        assert_eq!(js_to_fixed(2.5, 0), "3");
        assert_eq!(js_to_fixed(1.25, 1), "1.3");
        assert_eq!(js_to_fixed(1.005, 2), "1.00");
        assert_eq!(js_to_fixed(9.95, 1), "9.9");
        assert_eq!(js_to_fixed(9.75, 1), "9.8");
        assert_eq!(js_to_fixed(99.5, 0), "100");
        assert_eq!(js_to_fixed(16.6666, 3), "16.667");
        assert_eq!(js_to_fixed(-0.04, 1), "-0.0");
    }

    #[test]
    fn strings_are_escaped() {
        let mut out = String::new();
        write_string(&mut out, "a\"b\\c\n·");
        assert_eq!(out, "\"a\\\"b\\\\c\\n·\"");
    }
}
