//! Campaign unlock and star persistence (the `localStorage` keys of the original), with an
//! in-memory fallback when the browser storage fails.
use crate::small_map::SmallMap;

use crate::utils::clamp;

const HIGHEST_UNLOCKED_LEVEL_STORAGE_KEY: &str = "vector-defence-2026:highest-unlocked-level:v1";
const CAMPAIGN_CLEARED_STORAGE_KEY: &str = "vector-defence-2026:campaign-cleared:v1";
const LEVEL_STARS_STORAGE_KEY_PREFIX: &str = "vector-defence-2026:level-stars:v1:";

/// Key/value storage (`localStorage` in the browser).
pub trait ProgressStorage {
    fn read(&mut self, key: &str) -> Option<String>;
    fn write(&mut self, key: &str, value: &str);
    fn remove(&mut self, key: &str);
    /// True once an access has failed (a browser storage exception). The store then stops
    /// using this storage and keeps going from memory, like the TS `catch` branches.
    fn failed(&self) -> bool {
        false
    }
}

/// Plain in-memory storage (tests, and pages without `localStorage`).
#[derive(Clone, Debug, Default)]
pub struct MemoryProgressStorage {
    pub values: SmallMap<String, String>,
}

impl ProgressStorage for MemoryProgressStorage {
    fn read(&mut self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }

    fn write(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
    }

    fn remove(&mut self, key: &str) {
        self.values.remove(key);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignProgress {
    pub highest_unlocked_level_index: usize,
    pub campaign_cleared: bool,
}

pub struct CampaignProgressStore {
    memory: SmallMap<String, String>,
    storage: Option<Box<dyn ProgressStorage>>,
}

impl CampaignProgressStore {
    /// `None` keeps progress in memory only.
    pub fn new(storage: Option<Box<dyn ProgressStorage>>) -> CampaignProgressStore {
        CampaignProgressStore { memory: SmallMap::new(), storage }
    }

    pub fn load_campaign_progress(&mut self, level_count: usize) -> CampaignProgress {
        let saved_level_index = self.read(HIGHEST_UNLOCKED_LEVEL_STORAGE_KEY);
        let campaign_cleared = self.read(CAMPAIGN_CLEARED_STORAGE_KEY).as_deref() == Some("true");
        let highest_unlocked_level_index = saved_level_index.as_deref().map_or(0.0, js_number);
        let final_campaign_level_index = level_count.saturating_sub(1);
        let highest_unlocked_level_index = if campaign_cleared {
            final_campaign_level_index
        } else if is_integer(highest_unlocked_level_index) {
            clamp(highest_unlocked_level_index, 0.0, final_campaign_level_index as f64) as usize
        } else {
            0
        };
        CampaignProgress { highest_unlocked_level_index, campaign_cleared }
    }

    pub fn load_level_stars(&mut self, level_count: usize) -> Vec<u32> {
        (0..level_count)
            .map(|index| {
                let saved_stars = self.read(&level_stars_key(index)).as_deref().map_or(0.0, js_number);
                if is_integer(saved_stars) { clamp(saved_stars, 0.0, 3.0) as u32 } else { 0 }
            })
            .collect()
    }

    pub fn save_campaign_progress(&mut self, highest_unlocked_level_index: usize, campaign_cleared: bool) {
        self.write(HIGHEST_UNLOCKED_LEVEL_STORAGE_KEY, &highest_unlocked_level_index.to_string());
        if campaign_cleared {
            self.write(CAMPAIGN_CLEARED_STORAGE_KEY, "true");
        } else {
            self.remove(CAMPAIGN_CLEARED_STORAGE_KEY);
        }
    }

    pub fn save_level_stars(&mut self, level_index: usize, stars: u32) {
        self.write(&level_stars_key(level_index), &stars.to_string());
    }

    fn read(&mut self, key: &str) -> Option<String> {
        let memory_value = self.memory.get(key).cloned();
        let Some(storage) = self.storage.as_mut() else {
            return memory_value;
        };
        let value = storage.read(key);
        if storage.failed() {
            self.storage = None;
            return memory_value;
        }
        match &value {
            Some(value) => {
                self.memory.insert(key.to_string(), value.clone());
            }
            None => {
                self.memory.remove(key);
            }
        }
        value
    }

    fn write(&mut self, key: &str, value: &str) {
        self.memory.insert(key.to_string(), value.to_string());
        if let Some(storage) = self.storage.as_mut() {
            storage.write(key, value);
            if storage.failed() {
                self.storage = None;
            }
        }
    }

    fn remove(&mut self, key: &str) {
        self.memory.remove(key);
        if let Some(storage) = self.storage.as_mut() {
            storage.remove(key);
            if storage.failed() {
                self.storage = None;
            }
        }
    }
}

fn level_stars_key(index: usize) -> String {
    format!("{LEVEL_STARS_STORAGE_KEY_PREFIX}{index}")
}

fn is_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0
}

/// JavaScript `Number(text)` for stored strings: trimmed decimal, `Infinity`, and 0x/0o/0b
/// literals; anything else is NaN, and an empty string is 0.
fn js_number(text: &str) -> f64 {
    let trimmed = text.trim_matches(|character: char| character.is_ascii_whitespace());
    if trimmed.is_empty() {
        return 0.0;
    }
    match trimmed {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = trimmed.strip_prefix(prefix) {
            return u64::from_str_radix(digits, radix).map_or(f64::NAN, |value| value as f64);
        }
    }
    if !trimmed.chars().all(|character| matches!(character, '0'..='9' | '+' | '-' | '.' | 'e' | 'E')) {
        return f64::NAN;
    }
    parse_decimal(trimmed).unwrap_or(f64::NAN)
}

/// `[+-]digits[.digits][(e|E)[+-]digits]` (at least one digit before or after the point),
/// without the standard library's float parser and its tables. Stored progress values are
/// small integers, so the last-bit rounding of long mantissas does not matter here.
fn parse_decimal(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let negative = match bytes.first() {
        Some(b'-') => {
            index += 1;
            true
        }
        Some(b'+') => {
            index += 1;
            false
        }
        _ => false,
    };
    let mut mantissa = 0.0_f64;
    let mut exponent = 0_i32;
    let mut digits = 0;
    while let Some(&byte) = bytes.get(index).filter(|byte| byte.is_ascii_digit()) {
        mantissa = mantissa * 10.0 + f64::from(byte - b'0');
        digits += 1;
        index += 1;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while let Some(&byte) = bytes.get(index).filter(|byte| byte.is_ascii_digit()) {
            mantissa = mantissa * 10.0 + f64::from(byte - b'0');
            exponent -= 1;
            digits += 1;
            index += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        let exponent_negative = match bytes.get(index) {
            Some(b'-') => {
                index += 1;
                true
            }
            Some(b'+') => {
                index += 1;
                false
            }
            _ => false,
        };
        let start = index;
        let mut value = 0_i32;
        while let Some(&byte) = bytes.get(index).filter(|byte| byte.is_ascii_digit()) {
            value = value.saturating_mul(10).saturating_add(i32::from(byte - b'0'));
            index += 1;
        }
        if index == start {
            return None;
        }
        exponent = exponent.saturating_add(if exponent_negative { -value } else { value });
    }
    if index != bytes.len() {
        return None;
    }
    let magnitude = if exponent < 0 { mantissa / 10f64.powi(-exponent) } else { mantissa * 10f64.powi(exponent) };
    Some(if negative { -magnitude } else { magnitude })
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::*;

    #[test]
    fn parses_like_javascript_number() {
        assert_eq!(js_number(""), 0.0);
        assert_eq!(js_number(" 3 "), 3.0);
        assert_eq!(js_number("3.0"), 3.0);
        assert_eq!(js_number("1e1"), 10.0);
        assert_eq!(js_number("0x10"), 16.0);
        assert!(js_number("abc").is_nan());
        assert!(js_number("inf").is_nan());
        assert!(js_number("2.5").fract() != 0.0);
    }

    #[test]
    fn clamps_and_rejects_saved_values() {
        let mut storage = MemoryProgressStorage::default();
        storage.write(HIGHEST_UNLOCKED_LEVEL_STORAGE_KEY, "42");
        storage.write(&level_stars_key(0), "7");
        storage.write(&level_stars_key(1), "1.5");
        storage.write(&level_stars_key(2), "2");
        let mut store = CampaignProgressStore::new(Some(Box::new(storage)));
        assert_eq!(
            store.load_campaign_progress(10),
            CampaignProgress { highest_unlocked_level_index: 9, campaign_cleared: false }
        );
        assert_eq!(&store.load_level_stars(10)[..3], &[3, 0, 2]);
    }

    /// Storage whose chosen operation throws once `fail` is set.
    struct FlakyStorage {
        data: Rc<RefCell<SmallMap<String, String>>>,
        fail: Rc<Cell<bool>>,
        failed_method: &'static str,
        failed: bool,
    }

    impl FlakyStorage {
        fn attempt(&mut self, method: &str) -> bool {
            if self.fail.get() && method == self.failed_method {
                self.failed = true;
                return false;
            }
            true
        }
    }

    impl ProgressStorage for FlakyStorage {
        fn read(&mut self, key: &str) -> Option<String> {
            if !self.attempt("getItem") {
                return None;
            }
            self.data.borrow().get(key).cloned()
        }
        fn write(&mut self, key: &str, value: &str) {
            if self.attempt("setItem") {
                self.data.borrow_mut().insert(key.to_string(), value.to_string());
            }
        }
        fn remove(&mut self, key: &str) {
            if self.attempt("removeItem") {
                self.data.borrow_mut().remove(key);
            }
        }
        fn failed(&self) -> bool {
            self.failed
        }
    }

    #[test]
    fn progress_survives_storage_failures_using_memory() {
        for failed_method in ["getItem", "setItem", "removeItem"] {
            let fail = Rc::new(Cell::new(false));
            let storage = FlakyStorage {
                data: Rc::new(RefCell::new(SmallMap::new())),
                fail: fail.clone(),
                failed_method,
                failed: false,
            };
            let mut store = CampaignProgressStore::new(Some(Box::new(storage)));
            store.save_campaign_progress(3, true);
            store.save_level_stars(2, 3);
            store.load_campaign_progress(10);
            fail.set(true);
            if failed_method == "getItem" {
                store.load_campaign_progress(10);
            }
            store.save_campaign_progress(4, false);
            let progress = store.load_campaign_progress(10);
            assert_eq!(progress.highest_unlocked_level_index, 4, "{failed_method}");
            assert!(!progress.campaign_cleared, "{failed_method}");
            assert_eq!(store.load_level_stars(10)[2], 3, "{failed_method}");
        }
    }
}
