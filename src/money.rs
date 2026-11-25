use std::collections::HashMap;

use rust_decimal::{Decimal, RoundingStrategy};

/// Half away from zero, to the cent. Rows store this. Totals round at display.
pub(crate) fn usd_cents(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Unsigned integer of any size, little-endian 32-bit limbs, no leading zero limb.
/// Money is summed and divided in integers so nothing is cut before the one rounding.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Big(Vec<u32>);

impl Big {
    fn trim(mut self) -> Self {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
        self
    }

    pub(crate) fn from_u128(mut value: u128) -> Self {
        let mut limbs = Vec::new();
        while value > 0 {
            limbs.push(value as u32);
            value >>= 32;
        }
        Self(limbs)
    }

    fn to_u128(&self) -> Option<u128> {
        if self.0.len() > 4 {
            return None;
        }
        Some(
            self.0
                .iter()
                .rev()
                .fold(0, |acc, &limb| (acc << 32) | u128::from(limb)),
        )
    }

    fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    fn bits(&self) -> usize {
        match self.0.last() {
            None => 0,
            Some(top) => self.0.len() * 32 - top.leading_zeros() as usize,
        }
    }

    fn cmp_big(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }

    pub(crate) fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self(Vec::new());
        }
        let mut out = vec![0_u32; self.0.len() + other.0.len()];
        for (i, &a) in self.0.iter().enumerate() {
            let mut carry = 0_u64;
            for (j, &b) in other.0.iter().enumerate() {
                let cur = u64::from(out[i + j]) + u64::from(a) * u64::from(b) + carry;
                out[i + j] = cur as u32;
                carry = cur >> 32;
            }
            out[i + other.0.len()] = carry as u32;
        }
        Self(out).trim()
    }

    fn add(&self, other: &Self) -> Self {
        let len = self.0.len().max(other.0.len());
        let mut out = Vec::with_capacity(len + 1);
        let mut carry = 0_u64;
        for i in 0..len {
            let cur = u64::from(*self.0.get(i).unwrap_or(&0))
                + u64::from(*other.0.get(i).unwrap_or(&0))
                + carry;
            out.push(cur as u32);
            carry = cur >> 32;
        }
        out.push(carry as u32);
        Self(out).trim()
    }

    /// `self - other`, which must not be negative.
    fn sub(&self, other: &Self) -> Self {
        let mut out = self.0.clone();
        let mut borrow = 0_i64;
        for (i, limb) in out.iter_mut().enumerate() {
            let cur = i64::from(*limb) - i64::from(*other.0.get(i).unwrap_or(&0)) - borrow;
            borrow = i64::from(cur < 0);
            *limb = cur.rem_euclid(1 << 32) as u32;
        }
        Self(out).trim()
    }

    fn zero() -> Self {
        Self(Vec::new())
    }

    fn one() -> Self {
        Self(vec![1])
    }

    fn shl_limbs(&self, limbs: usize) -> Self {
        if self.0.is_empty() || limbs == 0 {
            return self.clone();
        }
        let mut out = vec![0_u32; limbs];
        out.extend_from_slice(&self.0);
        Self(out)
    }

    fn mul_limb(&self, limb: u32) -> Self {
        if limb == 0 || self.0.is_empty() {
            return Self::zero();
        }
        let mut out = Vec::with_capacity(self.0.len() + 1);
        let mut carry = 0_u64;
        for &a in &self.0 {
            let cur = u64::from(a) * u64::from(limb) + carry;
            out.push(cur as u32);
            carry = cur >> 32;
        }
        if carry != 0 {
            out.push(carry as u32);
        }
        Self(out)
    }

    /// Quotient and remainder. `divisor` must not be zero.
    /// One limb of the quotient at a time, so a row stays proportional to its digits.
    fn divmod(&self, divisor: &Self) -> (Self, Self) {
        if self.cmp_big(divisor) == std::cmp::Ordering::Less {
            return (Self::zero(), self.clone());
        }
        if divisor.0.len() == 1 {
            return self.div_limb(divisor.0[0]);
        }
        let q_len = self.0.len() - divisor.0.len() + 1;
        let mut quotient = vec![0_u32; q_len];
        let mut rest = self.clone();
        for qi in (0..q_len).rev() {
            let base = divisor.shl_limbs(qi);
            if base.cmp_big(&rest) == std::cmp::Ordering::Greater {
                continue;
            }
            let mut lo = 0_u32;
            let mut hi = u32::MAX;
            while lo < hi {
                let step = (u64::from(hi) - u64::from(lo)).div_ceil(2);
                let mid = u32::try_from(u64::from(lo) + step).expect("quotient digit");
                if base.mul_limb(mid).cmp_big(&rest) != std::cmp::Ordering::Greater {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            if lo != 0 {
                rest = rest.sub(&base.mul_limb(lo));
                quotient[qi] = lo;
            }
        }
        (Self(quotient).trim(), rest)
    }

    fn div_limb(&self, divisor: u32) -> (Self, Self) {
        let mut rest = 0_u64;
        let mut quotient = vec![0_u32; self.0.len()];
        for (i, &limb) in self.0.iter().enumerate().rev() {
            let cur = (rest << 32) | u64::from(limb);
            let d = u64::from(divisor);
            quotient[i] = (cur / d) as u32;
            rest = cur % d;
        }
        let rem = if rest == 0 {
            Self::zero()
        } else {
            Self(vec![rest as u32])
        };
        (Self(quotient).trim(), rem)
    }

    fn gcd(&self, other: &Self) -> Self {
        let (mut a, mut b) = (self.clone(), other.clone());
        while !b.is_zero() {
            let rest = a.divmod(&b).1;
            a = b;
            b = rest;
        }
        a
    }
}

/// An exact non-negative amount, `num / den`, in dollars.
#[derive(Clone, Debug)]
pub(crate) struct Frac {
    num: Big,
    den: Big,
}

impl Frac {
    pub(crate) fn new(num: Big, den: Big) -> Self {
        Self { num, den }
    }

    pub(crate) fn zero() -> Self {
        Self::new(Big::zero(), Big::from_u128(1))
    }

    /// Cancel shared factors so a later total does not keep an unreduced denominator.
    fn reduced(&self) -> Self {
        if self.num.is_zero() {
            return Self::zero();
        }
        let gcd = self.num.gcd(&self.den);
        if gcd.0.len() == 1 && gcd.0[0] == 1 {
            return self.clone();
        }
        Self {
            num: self.num.divmod(&gcd).0,
            den: self.den.divmod(&gcd).0,
        }
    }

    /// Dollars to whole cents, half away from zero, as a `Decimal`.
    pub(crate) fn round_cents(&self) -> Result<Decimal, crate::error::Error> {
        // floor((200 * num + den) / (2 * den))
        let numer = self.num.mul(&Big::from_u128(200)).add(&self.den);
        let (cents, _) = numer.divmod(&self.den.mul(&Big::from_u128(2)));
        cents_to_decimal(&cents)
    }
}

fn cents_to_decimal(cents: &Big) -> Result<Decimal, crate::error::Error> {
    const LIMIT: u128 = 1 << 96;
    let overflow = || crate::error::Error::new("price overflow");
    let cents = cents.to_u128().ok_or_else(overflow)?;
    if cents < LIMIT {
        return Decimal::try_from_i128_with_scale(cents as i128, 2).map_err(|_| overflow());
    }
    if cents % 100 == 0 && cents / 100 < LIMIT {
        return Decimal::try_from_i128_with_scale((cents / 100) as i128, 0).map_err(|_| overflow());
    }
    Err(overflow())
}

/// Sum of exact amounts, rounded to the cent once. The rows are added as scaled integers
/// with a known error of less than one unit per row. A sum that close to a half cent is
/// decided from the exact amounts: rows with the same denominator are added into one
/// numerator as they come in, so work and memory follow the number of different
/// denominators, not the number of rows. Those few are reduced and added over their least
/// common multiple, within a fixed limit. Past the limit the total is an error.
#[derive(Debug)]
pub(crate) struct Sum {
    scaled: Big,
    rows: u64,
    /// Numerator per denominator, `None` once there were more than `MAX_DENOMINATORS`.
    terms: Option<HashMap<Big, Big>>,
}

/// Units per dollar of the scaled sum.
const SUM_SCALE_DIGITS: u32 = 24;

/// Most different denominators the exact decision works over. Each costs one reduction and
/// one step over the common multiple, so the work is at most this many fixed-size steps.
const MAX_DENOMINATORS: usize = 4_096;

/// Widest common denominator of the exact decision, in bits.
const EXACT_SUM_BITS: usize = 2_048;

impl Sum {
    pub(crate) fn new() -> Self {
        Self {
            scaled: Big(Vec::new()),
            rows: 0,
            terms: Some(HashMap::new()),
        }
    }

    pub(crate) fn add(&mut self, amount: &Frac) {
        if amount.num.is_zero() {
            return;
        }
        let scale = Big::from_u128(10_u128.pow(SUM_SCALE_DIGITS));
        let (floor, _) = amount.num.mul(&scale).divmod(&amount.den);
        self.scaled = self.scaled.add(&floor);
        self.rows += 1;
        if let Some(terms) = &mut self.terms {
            match terms.get_mut(&amount.den) {
                Some(num) => *num = num.add(&amount.num),
                None => {
                    terms.insert(amount.den.clone(), amount.num.clone());
                }
            }
            if terms.len() > MAX_DENOMINATORS {
                self.terms = None;
            }
        }
    }

    pub(crate) fn round_cents(&self) -> Result<Decimal, crate::error::Error> {
        if let Some(cents) = self.scaled_cents() {
            return cents_to_decimal(&cents);
        }
        let too_close = || {
            crate::error::Error::new(format!(
                "total is too close to a half cent to round exactly within the work limit \
                 (more than {MAX_DENOMINATORS} different denominators, or a common denominator \
                 wider than {EXACT_SUM_BITS} bits)"
            ))
        };
        let terms = self.terms.as_ref().ok_or_else(too_close)?;
        match add_over_common_multiple(terms) {
            Some(sum) => sum.round_cents(),
            None => Err(too_close()),
        }
    }

    /// `Some` when every value inside the truncation interval rounds to the same cent.
    fn scaled_cents(&self) -> Option<Big> {
        let cent = Big::from_u128(10_u128.pow(SUM_SCALE_DIGITS - 2));
        let half = Big::from_u128(10_u128.pow(SUM_SCALE_DIGITS - 2) / 2);
        let low = self.scaled.add(&half).divmod(&cent).0;
        let high = self
            .scaled
            .add(&half)
            .add(&Big::from_u128(u128::from(self.rows)))
            .divmod(&cent)
            .0;
        if low == high {
            Some(low)
        } else {
            None
        }
    }
}

/// Exact sum over the least common multiple of the reduced denominators. `None` when that
/// multiple passes `EXACT_SUM_BITS`. The result is not reduced; it is only rounded.
fn add_over_common_multiple(terms: &HashMap<Big, Big>) -> Option<Frac> {
    let mut sum = Frac::new(Big::zero(), Big::one());
    for (den, num) in terms {
        let term = Frac::new(num.clone(), den.clone()).reduced();
        let gcd = sum.den.gcd(&term.den);
        let sum_scale = term.den.divmod(&gcd).0;
        let term_scale = sum.den.divmod(&gcd).0;
        sum = Frac {
            num: sum.num.mul(&sum_scale).add(&term.num.mul(&term_scale)),
            den: sum.den.mul(&sum_scale),
        };
        if sum.den.bits() > EXACT_SUM_BITS {
            return None;
        }
    }
    Some(sum)
}

/// `value / 10^scale` as integers: the mantissa and the power of ten.
pub(crate) fn decimal_parts(value: Decimal) -> (u128, u128) {
    (value.mantissa().unsigned_abs(), 10_u128.pow(value.scale()))
}

/// `bytes * rate / 1024^3` dollars, exactly. The rate is `mantissa / 10^scale`.
pub(crate) fn storage_frac(bytes: u64, rate: Decimal) -> Frac {
    let (mantissa, power) = decimal_parts(rate);
    let num = Big::from_u128(u128::from(bytes)).mul(&Big::from_u128(mantissa));
    Frac::new(num, Big::from_u128(power).mul(&Big::from_u128(1 << 30)))
}

/// GB column. At most 6 decimal places, half away from zero.
pub(crate) fn format_gb(value: Decimal) -> String {
    format_dec(value.round_dp_with_strategy(6, RoundingStrategy::MidpointAwayFromZero))
}

/// `stored_bytes / 1024^3`. The pricing examples treat 1 TB as 1,024 GB.
#[must_use]
pub fn gb_from_bytes(bytes: Decimal) -> Decimal {
    bytes / Decimal::from(1_073_741_824_u64)
}

/// Days since 1970-01-01 in the civil calendar. Howard Hinnant's algorithm.
/// `year`, `month`, and `day` must be a real Gregorian date.
#[must_use]
pub fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let mut y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Civil date (year, month, day) of a day count since 1970-01-01. Howard Hinnant's algorithm.
pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Midnight UTC of a real civil date, as Unix milliseconds.
#[must_use]
pub fn epoch_millis(year: i32, month: u32, day: u32) -> i64 {
    days_from_civil(year, month, day) * 86_400_000
}

pub(crate) fn ymd_to_epoch_millis(text: &str) -> Option<i64> {
    let (year, month, day) = parse_ymd(text)?;
    Some(epoch_millis(year, month, day))
}

pub(crate) fn parse_ymd(text: &str) -> Option<(i32, u32, u32)> {
    if !valid_date(text) {
        return None;
    }
    Some((
        text[0..4].parse().ok()?,
        text[5..7].parse().ok()?,
        text[8..10].parse().ok()?,
    ))
}

/// Decimal text with no exponent. Trailing zeros that `normalize` drops stay dropped.
#[must_use]
pub fn format_dec(value: Decimal) -> String {
    let text = value.normalize().to_string();
    if text.contains(['e', 'E']) {
        return value.round_dp(12).normalize().to_string();
    }
    text
}

/// Like [`format_dec`], rounded half away from zero to the cent.
#[must_use]
pub fn format_usd(value: Decimal) -> String {
    let text = format_dec(usd_cents(value));
    let Some((whole, frac)) = text.split_once('.') else {
        return format!("{text}.00");
    };
    if frac.len() >= 2 {
        text
    } else {
        format!("{whole}.{frac}{}", "0".repeat(2 - frac.len()))
    }
}

pub(crate) fn parse_money(raw: &str) -> Result<Decimal, ()> {
    let text = raw.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    if text.is_empty() || text.len() > 40 {
        return Err(());
    }
    let bytes = text.as_bytes();
    let mut dot = false;
    for (i, byte) in bytes.iter().enumerate() {
        match byte {
            b'0'..=b'9' => {}
            b'.' if !dot && i > 0 && i + 1 < bytes.len() => dot = true,
            _ => return Err(()),
        }
    }
    let (int_part, frac_part) = match text.split_once('.') {
        Some((int_part, frac)) => (int_part, Some(frac)),
        None => (text, None),
    };
    if frac_part.is_some_and(|frac| frac.len() > 28) {
        return Err(());
    }
    let parsed: Decimal = text.parse().map_err(|_| ())?;
    // `Decimal` keeps about 28 significant digits and rounds the rest on parse. A rate
    // that does not come back digit for digit would be a different rate than the one written.
    let int_part = match int_part.trim_start_matches('0') {
        "" => "0",
        trimmed => trimmed,
    };
    let canonical = match frac_part {
        Some(frac) => format!("{int_part}.{frac}"),
        None => int_part.to_string(),
    };
    if parsed.to_string() == canonical {
        Ok(parsed)
    } else {
        Err(())
    }
}

pub(crate) fn valid_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits_only = bytes
        .iter()
        .enumerate()
        .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    if !digits_only {
        return false;
    }
    let Ok(year) = text[0..4].parse::<i32>() else {
        return false;
    };
    let Ok(month) = text[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = text[8..10].parse::<u32>() else {
        return false;
    };
    if !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=max).contains(&day)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rust_decimal::Decimal;

    use super::{
        days_from_civil, epoch_millis, format_dec, format_gb, format_usd, gb_from_bytes,
        parse_money, valid_date, Big, Frac, Sum, MAX_DENOMINATORS,
    };

    fn frac(num: u128, den: u128) -> Frac {
        Frac::new(Big::from_u128(num), Big::from_u128(den))
    }

    #[test]
    fn big_arithmetic_matches_u128() {
        let a = 0xDEAD_BEEF_0123_4567_89AB_CDEF_u128;
        let b = 0x1_0000_0001_u128;
        let (q, r) = Big::from_u128(a).divmod(&Big::from_u128(b));
        assert_eq!(q.to_u128(), Some(a / b));
        assert_eq!(r.to_u128(), Some(a % b));
        assert_eq!(
            Big::from_u128(a).mul(&Big::from_u128(b)).to_u128(),
            a.checked_mul(b)
        );
        assert_eq!(
            Big::from_u128(a).add(&Big::from_u128(b)).to_u128(),
            Some(a + b)
        );
        assert_eq!(Big::from_u128(a).gcd(&Big::from_u128(b)).to_u128(), Some(1));
        assert_eq!(
            Big::from_u128(48).gcd(&Big::from_u128(36)).to_u128(),
            Some(12)
        );
        let wide = Big::from_u128(u128::MAX).mul(&Big::from_u128(u128::MAX));
        assert_eq!(wide.to_u128(), None);
        assert_eq!(
            wide.divmod(&Big::from_u128(u128::MAX)).0.to_u128(),
            Some(u128::MAX)
        );
        // Quotient and remainder of values that do not fit in one machine word.
        let scale = Big::from_u128(10_u128.pow(24));
        let (q, r) = wide.divmod(&scale);
        assert_eq!(q.mul(&scale).add(&r), wide);
        assert!(r.cmp_big(&scale) == std::cmp::Ordering::Less);
        let odd = wide.add(&Big::from_u128(123_456_789));
        let (q, r) = odd.divmod(&scale);
        assert_eq!(q.mul(&scale).add(&r), odd);
        assert!(r.cmp_big(&scale) == std::cmp::Ordering::Less);
        let (q, r) = scale.divmod(&wide);
        assert!(q.is_zero());
        assert_eq!(r, scale);
    }

    #[test]
    fn rows_with_one_denominator_add_into_one_numerator() {
        // 100000 rows of 1/1200 each: 83.333... dollars. One denominator, so the exact
        // decision is one step however many rows there are.
        let mut sum = Sum::new();
        for _ in 0..100_000 {
            sum.add(&frac(1, 1200));
        }
        assert_eq!(sum.terms.as_ref().map(HashMap::len), Some(1));
        assert_eq!(format_usd(sum.round_cents().unwrap()), "83.33");
        // 400 rows of 1/80000 are exactly 1/200: half a cent, so one cent.
        let mut half = Sum::new();
        for _ in 0..400 {
            half.add(&frac(1, 80_000));
        }
        assert_eq!(format_usd(half.round_cents().unwrap()), "0.01");
    }

    /// 1/200 minus 10^-27, as two rows: just under a half cent, so the fixed-point sum
    /// cannot tell which cent it is.
    fn just_under_a_half_cent() -> Sum {
        let mut sum = Sum::new();
        sum.add(&frac(1, 600));
        sum.add(&frac(10_u128.pow(27) - 300, 300 * 10_u128.pow(27)));
        sum
    }

    #[test]
    fn many_different_denominators_near_a_half_cent_are_refused_at_once() {
        // A hair of about 10^-30 for each of MAX_DENOMINATORS + 1 different denominators.
        // Too close to the half cent for the fixed-point sum and too many denominators
        // for the exact one, so the work is bounded and the answer is an error.
        let mut sum = just_under_a_half_cent();
        let base = 10_u128.pow(30);
        for i in 0..=(MAX_DENOMINATORS as u128) {
            sum.add(&frac(1, base + 2 * i + 1));
        }
        assert!(sum.terms.is_none());
        let err = sum.round_cents().unwrap_err();
        assert!(err
            .message()
            .starts_with("total is too close to a half cent"));
    }

    #[test]
    fn a_common_denominator_past_the_limit_is_refused() {
        // 200 denominators of about 100 bits each: their common multiple passes 2048 bits
        // long before the last one.
        let mut sum = just_under_a_half_cent();
        for i in 0..200_u128 {
            sum.add(&frac(1, (1_u128 << 100) + 2 * i + 1));
        }
        assert_eq!(sum.terms.as_ref().map(HashMap::len), Some(202));
        let err = sum.round_cents().unwrap_err();
        assert!(err.message().contains("wider than 2048 bits"));
    }

    #[test]
    fn a_few_different_denominators_near_a_half_cent_still_round_exactly() {
        // The same hairs, 12 of them: the common multiple stays inside the limit. The
        // total is 1/200 - 10^-27 + 12 * 10^-30, still under a half cent: 0.00.
        let mut sum = just_under_a_half_cent();
        for i in 0..12_u128 {
            sum.add(&frac(1, 10_u128.pow(30) + 2 * i + 1));
        }
        assert_eq!(format_usd(sum.round_cents().unwrap()), "0.00");
        // One hair of 2 * 10^-27 on top lifts it over: 0.01.
        sum.add(&frac(2, 10_u128.pow(27)));
        assert_eq!(format_usd(sum.round_cents().unwrap()), "0.01");
    }

    #[test]
    fn a_sum_of_fractions_with_other_denominators_rounds_the_exact_half_cent_up() {
        // 1/600 + 1/300 = 1/200 = 0.005 exactly; each part alone is cut when scaled.
        let mut sum = Sum::new();
        sum.add(&frac(1, 600));
        sum.add(&frac(1, 300));
        assert_eq!(format_usd(sum.round_cents().unwrap()), "0.01");
        // A hair under half a cent stays 0.00.
        let mut below = Sum::new();
        below.add(&frac(1, 600));
        below.add(&frac(10_u128.pow(27) - 300, 300 * 10_u128.pow(27)));
        assert_eq!(format_usd(below.round_cents().unwrap()), "0.00");
    }

    #[test]
    fn amounts_that_do_not_fit_are_an_error_not_a_cut_amount() {
        let whole = frac(79_228_162_514_264_337_593_543_950_335, 1);
        assert_eq!(
            format_usd(whole.round_cents().unwrap()),
            "79228162514264337593543950335.00"
        );
        let cents = frac(79_228_162_514_264_337_593_543_950_335, 11);
        assert_eq!(cents.round_cents().unwrap_err().message(), "price overflow");
        let huge = Frac::new(
            Big::from_u128(u128::MAX).mul(&Big::from_u128(u128::MAX)),
            Big::from_u128(1),
        );
        assert_eq!(huge.round_cents().unwrap_err().message(), "price overflow");
    }

    #[test]
    fn unix_epoch_and_a_non_leap_span() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(epoch_millis(1970, 1, 1), 0);
        let span = days_from_civil(2026, 11, 9) - days_from_civil(2025, 11, 9);
        assert_eq!(span, 365);
        assert_eq!(
            epoch_millis(2026, 11, 9) - epoch_millis(2025, 11, 9),
            365 * 86_400_000
        );
    }

    #[test]
    fn leap_day_span() {
        assert_eq!(
            days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 28),
            2
        );
        assert_eq!(
            days_from_civil(2025, 3, 1) - days_from_civil(2025, 2, 28),
            1
        );
    }

    #[test]
    fn one_gibibyte_and_the_page_terabyte() {
        let gib = Decimal::from(1_073_741_824_u64);
        assert_eq!(gb_from_bytes(gib), Decimal::ONE);
        let one_tb = gib * Decimal::from(1024);
        assert_eq!(gb_from_bytes(one_tb), Decimal::from(1024));
        let rate = "0.03".parse::<Decimal>().unwrap();
        assert_eq!(format_usd(gb_from_bytes(one_tb) * rate), "30.72");
        let thirty_tb = one_tb * Decimal::from(30);
        assert_eq!(format_usd(gb_from_bytes(thirty_tb) * rate), "921.60");
    }

    #[test]
    fn money_rejects_exponent_sign_and_empty() {
        assert!(parse_money("0.03").is_ok());
        assert!(parse_money("0").is_ok());
        assert!(parse_money("1e-7").is_err());
        assert!(parse_money("1E-7").is_err());
        assert!(parse_money("-1").is_err());
        assert!(parse_money("+1").is_err());
        assert!(parse_money(".2").is_err());
        assert!(parse_money("1.").is_err());
        assert!(parse_money("").is_err());
        assert!(parse_money("0.00000000000000000000000000001").is_err());
        assert!(parse_money("0.0000000000000000000000000001").is_ok());
        assert!(parse_money("1.005").is_ok());
    }

    #[test]
    fn money_must_round_trip_digit_for_digit() {
        // 28 significant digits are exact; a 29th would be rounded away.
        assert!(parse_money("12345678901234567890.12345678").is_ok());
        assert!(parse_money("1234567890123456789.012345678").is_ok());
        assert!(parse_money("1000000000000000000.0049999999999").is_err());
        assert!(parse_money("1000000000000000000.0040000000000").is_err());
        assert!(parse_money("0.0300000000000000000000000000").is_ok());
        // 40 characters at most.
        assert!(parse_money(&format!("{}0.03", "0".repeat(36))).is_ok());
        assert!(parse_money(&format!("{}0.03", "0".repeat(37))).is_err());
        assert!(parse_money("007.50").is_ok());
    }

    #[test]
    fn money_trims_only_ascii_whitespace() {
        use super::parse_money;
        assert!(parse_money(" \t\r\n1 ").is_ok());
        for ws in ["\u{a0}", "\u{2028}", "\u{3000}"] {
            assert!(parse_money(&format!("{ws}1")).is_err());
            assert!(parse_money(&format!("1{ws}")).is_err());
        }
    }

    #[test]
    fn usd_keeps_two_digits() {
        assert_eq!(format_usd(Decimal::ZERO), "0.00");
        assert_eq!(format_usd(Decimal::from(2)), "2.00");
        assert_eq!(format_usd("0.3".parse().unwrap()), "0.30");
        assert_eq!(format_dec("10.05".parse().unwrap()), "10.05");
    }

    #[test]
    fn usd_rounds_half_away_from_zero_to_the_cent() {
        assert_eq!(format_usd("0.005".parse().unwrap()), "0.01");
        assert_eq!(format_usd("0.014".parse().unwrap()), "0.01");
        assert_eq!(format_usd("0.015".parse().unwrap()), "0.02");
        assert_eq!(format_usd("0.025".parse().unwrap()), "0.03");
        assert_eq!(format_usd("1.235".parse().unwrap()), "1.24");
        let gb = gb_from_bytes(Decimal::from(123_456_789_u64));
        assert_eq!(format_gb(gb), "0.114978");
        assert!(!format_gb(gb).contains("0.1149780945852398872375488281"));
    }

    #[test]
    fn dates_follow_the_calendar() {
        assert!(valid_date("2026-11-09"));
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2025-02-29"));
        assert!(!valid_date("2026-13-01"));
        assert!(!valid_date("2026-00-01"));
        assert!(!valid_date("2026-04-31"));
        assert!(!valid_date("2026-11-9"));
        assert!(!valid_date("20261109"));
        assert!(valid_date("2000-02-29"));
        assert!(!valid_date("1900-02-29"));
    }

    /// Odd primes at and above `2^30`. Each is a different denominator, and `p/p` is one
    /// dollar, so the exact decision has to reduce every one of them.
    fn wide_primes() -> Vec<u128> {
        let mut primes = Vec::with_capacity(547);
        let mut n = (1_u128 << 30) | 1;
        while primes.len() < 547 {
            let mut prime = true;
            let mut d = 3_u128;
            while d * d <= n {
                if n % d == 0 {
                    prime = false;
                    break;
                }
                d += 2;
            }
            if prime {
                primes.push(n);
            }
            n += 2;
        }
        primes
    }

    fn sum_of_ones(primes: &[u128], extra: Frac) -> Sum {
        let mut sum = Sum::new();
        sum.add(&extra);
        for &prime in primes {
            sum.add(&frac(prime, prime));
        }
        sum
    }

    #[test]
    fn a_half_cent_sum_of_many_denominators_rounds_once() {
        // 547 primes above 2^30. Each `p/p` is one dollar. The product of the
        // denominators is wider than 16384 bits, which is the point where the
        // exact total used to fail closed.
        let primes = wide_primes();
        assert_eq!(primes.len(), 547);
        let pow = 10_u128.pow(30);

        // (10^30 - 200)/(200 * 10^30) + 1/10^30 = 1/200 exactly, so the total is
        // k + 0.005. Neither part is a multiple of 10^-24, and the prime
        // denominators make the exact total wider than 16384 bits.
        // Half a cent rounds away from zero: k.01.
        let mut up = sum_of_ones(&primes, frac(pow - 200, 200 * pow));
        up.add(&frac(1, pow));
        assert_eq!(
            format_usd(up.round_cents().expect("exact half cent fits")),
            "547.01"
        );

        // 1/200 - 1/10^30, just under a half cent, so the same dollars stay at k.00.
        let under = sum_of_ones(&primes, frac(pow - 200, 200 * pow));
        assert_eq!(
            format_usd(under.round_cents().expect("just under a half cent fits")),
            "547.00"
        );

        // One byte at 5368709.1199999999999999999999 USD/GB is
        // 53687091199999999999999999999 / 10737418240000000000000000000000,
        // which is 1/10^31 under a half cent. The total is k.00, not k.01 and not an error.
        let rate = sum_of_ones(
            &primes,
            frac(
                53_687_091_199_999_999_999_999_999_999,
                10_737_418_240_000_000_000_000_000_000_000,
            ),
        );
        assert_eq!(
            format_usd(rate.round_cents().expect("28-digit rate fits")),
            "547.00"
        );
    }
}
