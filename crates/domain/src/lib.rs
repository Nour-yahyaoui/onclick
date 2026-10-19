//! Pure domain logic — no I/O, no async, no database.
//!
//! Everything that touches money lives here so that the till, the control
//! plane and the tests compute the same number the same way.
//!
//! Rule (ADR-007 / DESIGN-002): money is `i64` **millimes** (1 TND = 1000).
//! Never floats. Never rounding hidden from the user.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub const MILLIMES_PER_UNIT: i64 = 1000;

// ---------------------------------------------------------------------------
// Money formatting / parsing
// ---------------------------------------------------------------------------

/// Groups the integer part with commas: `1234567` -> `1,234,567`.
fn group_thousands(units: i64) -> String {
    let s = units.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Format millimes with **3 decimals, always**, and a comma as thousands
/// separator: `1250500` -> `1,250.500`. Negative values keep a leading `-`.
pub fn format_money(millimes: i64) -> String {
    let negative = millimes < 0;
    let abs = millimes.unsigned_abs() as i64;
    let units = abs / MILLIMES_PER_UNIT;
    let frac = abs % MILLIMES_PER_UNIT;
    format!(
        "{}{}.{:03}",
        if negative { "-" } else { "" },
        group_thousands(units),
        frac
    )
}

/// Format with the currency suffix: `1,250.500 TND`.
pub fn format_money_tnd(millimes: i64) -> String {
    format!("{} TND", format_money(millimes))
}

/// Parse user input into millimes. Accepts `12`, `12.5`, `12,5`, `1 250.500`,
/// `1,250.500`. A trailing separator group of 1..=3 digits is read as decimals;
/// otherwise separators are treated as thousands.
pub fn parse_money(input: &str) -> Option<i64> {
    let mut s: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '_')
        .collect();
    if s.is_empty() {
        return None;
    }
    let negative = s.starts_with('-');
    s = s.trim_start_matches(['-', '+']).to_string();
    if s.is_empty() {
        return None;
    }

    let (int_s, frac_s) = match s.rfind(['.', ',']) {
        Some(i) => {
            let frac = &s[i + 1..];
            let head = &s[..i];
            let frac_ok = !frac.is_empty()
                && frac.len() <= 3
                && frac.chars().all(|c| c.is_ascii_digit())
                && !head.is_empty()
                && head.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',');
            if frac_ok {
                (
                    head.chars().filter(|c| c.is_ascii_digit()).collect::<String>(),
                    frac.to_string(),
                )
            } else {
                (
                    s.chars().filter(|c| c.is_ascii_digit()).collect::<String>(),
                    String::new(),
                )
            }
        }
        None => (
            s.chars().filter(|c| c.is_ascii_digit()).collect::<String>(),
            String::new(),
        ),
    };

    if int_s.is_empty() && frac_s.is_empty() {
        return None;
    }
    let int_val: i64 = if int_s.is_empty() {
        0
    } else {
        int_s.parse().ok()?
    };
    let mut frac = frac_s;
    while frac.len() < 3 {
        frac.push('0');
    }
    let frac_val: i64 = if frac.is_empty() { 0 } else { frac.parse().ok()? };
    let total = int_val.checked_mul(MILLIMES_PER_UNIT)?.checked_add(frac_val)?;
    Some(if negative { -total } else { total })
}

// ---------------------------------------------------------------------------
// Cart maths
// ---------------------------------------------------------------------------

/// One line as it will be stored on a sale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub qty: i64,
    /// What the customer actually agreed to pay (ADR-009: entered, not looked up).
    pub unit_price: i64,
    /// Cost of the lot consumed — frozen at sale time (ADR-008).
    pub unit_cost: i64,
}

impl Line {
    pub fn total(&self) -> i64 {
        self.qty.saturating_mul(self.unit_price)
    }

    pub fn cost_total(&self) -> i64 {
        self.qty.saturating_mul(self.unit_cost)
    }
}

pub fn cart_total(lines: &[Line]) -> i64 {
    lines.iter().map(Line::total).sum()
}

pub fn cart_cost(lines: &[Line]) -> i64 {
    lines.iter().map(Line::cost_total).sum()
}

/// Gross margin in basis points (0 = break-even). Used for the live hint under
/// the price field and for reports — same function everywhere.
pub fn margin_bp(revenue: i64, cost: i64) -> i64 {
    if revenue <= 0 {
        return -10_000;
    }
    let profit = revenue - cost;
    // profit / revenue * 10_000, integer maths, no float drift.
    (profit.saturating_mul(10_000)) / revenue
}

pub fn margin_pct_label(bp: i64) -> String {
    let whole = bp / 100;
    let frac = (bp % 100).abs();
    format!("{}{}.{:02}%", if bp < 0 { "-" } else { "" }, whole.abs(), frac)
}

// ---------------------------------------------------------------------------
// Tax (ADR-007: off by default, never required)
// ---------------------------------------------------------------------------

/// Extract the pre-tax amount from a tax-inclusive total.
/// `rate_bp` is basis points: 1900 = 19 %.
pub fn ht_from_ttc(ttc: i64, rate_bp: i64) -> i64 {
    if rate_bp <= 0 {
        return ttc;
    }
    let divisors = 10_000 + rate_bp;
    (ttc.saturating_mul(10_000)) / divisors
}

pub fn vat_from_ttc(ttc: i64, rate_bp: i64) -> i64 {
    ttc - ht_from_ttc(ttc, rate_bp)
}

// ---------------------------------------------------------------------------
// Day helpers
// ---------------------------------------------------------------------------

/// Business day key used by reports and sync: `YYYY-MM-DD`.
pub fn day_key_named() -> &'static str {
    // Kept dependency-free; the storage layer owns real clock access so the
    // domain crate stays pure and testable.
    "1970-01-01"
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_three_decimals_always() {
        assert_eq!(format_money(0), "0.000");
        assert_eq!(format_money(5), "0.005");
        assert_eq!(format_money(1000), "1.000");
        assert_eq!(format_money(1250500), "1,250.500");
        assert_eq!(format_money(-1250500), "-1,250.500");
        assert_eq!(format_money(999_999_999), "999,999.999");
    }

    #[test]
    fn parses_shop_input() {
        assert_eq!(parse_money("12"), Some(12_000));
        assert_eq!(parse_money("12.5"), Some(12_500));
        assert_eq!(parse_money("12,5"), Some(12_500));
        assert_eq!(parse_money("1 250.500"), Some(1_250_500));
        assert_eq!(parse_money("1,250.500"), Some(1_250_500));
        assert_eq!(parse_money("1.250,5"), Some(1_250_500)); // European style
        assert_eq!(parse_money(""), None);
        assert_eq!(parse_money("abc"), None);
    }

    #[test]
    fn roundtrip_parse_format() {
        for v in [0i64, 1, 999, 1000, 12_500, 1_250_500, 987_654_321] {
            assert_eq!(parse_money(&format_money(v)), Some(v));
        }
    }

    #[test]
    fn cart_and_margin() {
        let lines = vec![
            Line { qty: 2, unit_price: 150_000, unit_cost: 120_000 },
            Line { qty: 1, unit_price: 60_000, unit_cost: 25_000 },
        ];
        assert_eq!(cart_total(&lines), 360_000);
        assert_eq!(cart_cost(&lines), 265_000);
        assert_eq!(margin_bp(360_000, 265_000), 2638); // 26.38 %
        assert_eq!(margin_pct_label(2638), "26.38%");
    }

    #[test]
    fn vat_split() {
        // 119.000 TTC at 19 % -> 100.000 HT
        assert_eq!(ht_from_ttc(119_000, 1900), 100_000);
        assert_eq!(vat_from_ttc(119_000, 1900), 19_000);
        // No tax configured -> total is the total.
        assert_eq!(ht_from_ttc(119_000, 0), 119_000);
    }
}
