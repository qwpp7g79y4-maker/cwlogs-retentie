//! Offline estimate of CloudWatch Logs storage cost from `describe-log-groups` JSON.
//!
//! The library reads bytes you already have. It does not open a socket, it does
//! not link an AWS SDK, and it does not run `put-retention-policy`.

#![forbid(unsafe_code)]

mod bill;
mod class;
mod error;
mod json_scan;
mod money;
mod parse;
mod price;
mod report;
mod run;

pub use bill::{horizon_days, savings_usd, storage_usd, Bill, Row, Totals};
pub use class::Class;
pub use error::{escape_controls, escape_os_str, Error};
pub use money::{days_from_civil, epoch_millis, format_dec, format_usd, gb_from_bytes};
pub use parse::{is_cloudwatch_retention, parse_document, valid_log_group_name, Document, Group};
pub use price::{load_prices_str, prices_builtin, Prices};
pub use report::{
    render, render_with, ESTIMATE_LINE, FREE_TIER_LINE, NOT_PRICED_LINE, SAVINGS_NOTE,
    SCHEMA_VERSION,
};
pub use run::{
    bill_from_inputs, execute_paths, load_prices_file, price_age_warning, NamedBytes, Options,
    Output, PRICE_MAX_AGE_DAYS,
};

/// Largest input accepted per file, in bytes (256 MiB).
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
