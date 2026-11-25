use rust_decimal::Decimal;

use crate::class::Class;
use crate::error::Error;
use crate::money::{decimal_parts, storage_frac, Big, Frac};
use crate::price::Prices;

/// Monthly storage USD of the bytes already stored, rounded half away from zero
/// to the cent. One GB is 1024^3 bytes. The amount is worked out in integers and
/// rounded once. An amount that does not fit is an input error.
pub fn storage_usd(stored_bytes: u64, rate: Decimal) -> Result<Decimal, Error> {
    storage_frac(stored_bytes, rate).round_cents()
}

/// A number of days as an exact fraction, `num / den`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Horizon {
    pub(crate) num: u128,
    pub(crate) den: u128,
}

impl Horizon {
    fn from_decimal(days: Decimal) -> Self {
        let (num, den) = decimal_parts(days);
        Self { num, den }
    }
}

/// How many days of logs are still in the snapshot.
///
/// No retention: the age, because nothing has expired.
/// With retention: the shorter of the age and the retention. A target at or
/// above this horizon does not delete any of the bytes in the snapshot.
#[must_use]
pub fn horizon_days(age_days: Decimal, retention_days: Option<u32>) -> Decimal {
    match retention_days {
        Some(days) => {
            let cap = Decimal::from(days);
            if age_days < cap {
                age_days
            } else {
                cap
            }
        }
        None => age_days,
    }
}

/// USD per month removed from the snapshot if retention becomes `target_days`
/// and ingest has been steady, rounded half away from zero to the cent.
/// `None` when the horizon is zero and bytes remain, because the mix of ages
/// is unknown.
pub fn savings_usd(
    stored_bytes: u64,
    horizon: Decimal,
    target_days: u32,
    rate: Decimal,
) -> Result<Option<Decimal>, Error> {
    if horizon.is_sign_negative() && !horizon.is_zero() {
        // A negative horizon is below any target: nothing is removed.
        return Ok(Some(Decimal::ZERO));
    }
    let horizon = Horizon::from_decimal(horizon);
    savings_frac(stored_bytes, horizon, target_days, rate)?
        .map(|frac| frac.round_cents())
        .transpose()
}

/// Exact savings. Totals add these amounts and round once when shown.
pub(crate) fn savings_frac(
    stored_bytes: u64,
    horizon: Horizon,
    target_days: u32,
    rate: Decimal,
) -> Result<Option<Frac>, Error> {
    if horizon.num == 0 {
        return Ok(if stored_bytes == 0 {
            Some(Frac::zero())
        } else {
            None
        });
    }
    let target = u128::from(target_days) * horizon.den;
    if target >= horizon.num {
        return Ok(Some(Frac::zero()));
    }
    // saved = stored * (1 - target / horizon) = stored * (num - target * den) / num
    let (mantissa, power) = decimal_parts(rate);
    let num = Big::from_u128(u128::from(stored_bytes))
        .mul(&Big::from_u128(horizon.num - target))
        .mul(&Big::from_u128(mantissa));
    let den = Big::from_u128(horizon.num)
        .mul(&Big::from_u128(power))
        .mul(&Big::from_u128(1 << 30));
    Ok(Some(Frac::new(num, den)))
}

/// One priced log group.
#[derive(Debug)]
pub struct Row {
    pub name: String,
    pub class: Class,
    pub retention_days: Option<u32>,
    pub age_days: Option<Decimal>,
    pub stored_bytes: u64,
    pub gb: Decimal,
    pub usd_month: Decimal,
    pub save_30_usd: Option<Decimal>,
    pub save_90_usd: Option<Decimal>,
    pub save_365_usd: Option<Decimal>,
}

/// Sum of the unrounded amounts. Shown totals are rounded to the cent, so
/// rounded rows do not add up to the total. A savings total is absent when any
/// row omitted savings.
#[derive(Debug)]
pub struct Totals {
    pub stored_bytes: u64,
    pub gb: Decimal,
    pub usd_month: Decimal,
    pub save_30_usd: Option<Decimal>,
    pub save_90_usd: Option<Decimal>,
    pub save_365_usd: Option<Decimal>,
}

#[derive(Debug)]
pub struct Bill {
    pub prices: Prices,
    pub as_of: Option<String>,
    pub fail_stored_bytes: Option<u64>,
    pub threshold_exceeded: bool,
    pub rows: Vec<Row>,
    pub warnings: Vec<String>,
    pub totals: Totals,
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{horizon_days, savings_usd, storage_usd};

    fn rate() -> Decimal {
        "0.03".parse().unwrap()
    }

    #[test]
    fn page_thirty_terabytes_is_921_60() {
        let bytes = 30 * 1024_u64.pow(4);
        assert_eq!(bytes, 32_985_348_833_280);
        assert_eq!(format_usd(storage_usd(bytes, rate()).unwrap()), "921.60");
    }

    #[test]
    fn six_gibibytes_do_not_use_the_five_gb_waiver() {
        let bytes = 6 * 1_073_741_824;
        assert_eq!(format_usd(storage_usd(bytes, rate()).unwrap()), "0.18");
    }

    #[test]
    fn horizon_is_the_shorter_of_age_and_retention() {
        let age = Decimal::from(100);
        assert_eq!(horizon_days(age, None), age);
        assert_eq!(horizon_days(age, Some(50)), Decimal::from(50));
        assert_eq!(horizon_days(age, Some(400)), age);
        let stored = 100 * 1_073_741_824;
        let horizon = horizon_days(age, Some(50));
        assert_eq!(
            format_usd(savings_usd(stored, horizon, 30, rate()).unwrap().unwrap()),
            "1.20"
        );
        assert_eq!(
            savings_usd(stored, horizon, 90, rate()).unwrap().unwrap(),
            Decimal::ZERO
        );
        assert_eq!(
            format_usd(savings_usd(stored, age, 30, rate()).unwrap().unwrap()),
            "2.10"
        );
    }

    #[test]
    fn zero_horizon_with_bytes_has_no_savings() {
        assert!(savings_usd(5, Decimal::ZERO, 30, rate()).unwrap().is_none());
        assert_eq!(
            savings_usd(0, Decimal::ZERO, 30, rate()).unwrap().unwrap(),
            Decimal::ZERO
        );
        assert_eq!(
            savings_usd(1_073_741_824, Decimal::from(10), 30, rate())
                .unwrap()
                .unwrap(),
            Decimal::ZERO
        );
    }

    fn format_usd(value: Decimal) -> String {
        crate::money::format_usd(value)
    }
}
