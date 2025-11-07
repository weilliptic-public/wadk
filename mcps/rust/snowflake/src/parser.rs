use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};
use serde_json::Value;
use weil_rs::utils::cleanse_input_string;

pub fn cleanse_sql(input: &str) -> String {
    cleanse_input_string(input)
}

fn parse_snowflake_date(s: &str) -> Option<NaiveDate> {
    // Snowflake stores date as days since 1970-01-01
    let days_since_epoch = s.parse::<i64>().ok()?;
    let unix_epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    if days_since_epoch >= 0 {
        unix_epoch.checked_add_signed(TimeDelta::days(days_since_epoch))
    } else {
        unix_epoch.checked_sub_signed(TimeDelta::days(days_since_epoch.unsigned_abs() as i64))
    }
}

fn parse_snowflake_time(s: &str, scale: i64) -> Option<NaiveTime> {
    let scale_factor = 10i32.pow(scale as u32);
    let mut v = s.parse::<f64>().ok()?;
    v *= scale_factor as f64;
    let secs = (v.trunc() / scale_factor as f64) as u32;
    let nsec = (v.fract() * 10_f64.powi(9 - scale as i32)) as u32;
    NaiveTime::from_num_seconds_from_midnight_opt(secs, nsec)
}

fn parse_snowflake_timestamp_ntz_ltz(s: &str, scale: i64) -> Option<NaiveDateTime> {
    let scale_factor = 10i32.pow(scale as u32);
    let mut v = s.parse::<f64>().ok()?;
    v *= scale_factor as f64;
    let secs = v.trunc() as i64 / scale_factor as i64;
    let nsec = (v.fract() * 10_f64.powi(9 - scale as i32)) as u32;
    chrono::DateTime::from_timestamp(secs, nsec)?
        .naive_utc()
        .into()
}

fn parse_snowflake_timestamp_tz(s: &str, scale: i64) -> Option<NaiveDateTime> {
    // Try Result version 0 (timezone baked in)
    if let Ok(v) = s.parse::<f64>() {
        let scale_factor = 10i32.pow(scale as u32);
        let frac_secs_with_tz = v * scale_factor as f64;
        let frac_secs = frac_secs_with_tz / 16384.;
        let mut min_addend = frac_secs_with_tz as i64 % 16384;
        if min_addend < 0 {
            min_addend += 16384;
        }
        let secs = frac_secs.trunc() as i64 / scale_factor as i64;
        let nsec = (frac_secs.fract() * 10_f64.powi(9 - scale as i32)) as u32;
        let dt = chrono::DateTime::from_timestamp(secs, nsec)?.naive_utc();
        return dt.checked_add_signed(TimeDelta::minutes(min_addend));
    }
    // Result version > 0 (space separated value and tz)
    let pair: Vec<_> = s.split_whitespace().collect();
    let v = pair.first()?.parse::<f64>().ok()?;
    let scale_factor = 10i32.pow(scale as u32);
    let v = v * scale_factor as f64;
    let secs = v.trunc() as i64 / scale_factor as i64;
    let nsec = (v.fract() * 10_f64.powi(9 - scale as i32)) as u32;
    let dt = chrono::DateTime::from_timestamp(secs, nsec)?.naive_utc();
    let tz = pair.get(1)?.parse::<i64>().ok()?;
    if !(0..=2880).contains(&tz) {
        return None;
    }
    let min_addend = 1440 - tz;
    dt.checked_add_signed(TimeDelta::minutes(min_addend))
}

/// Parses a Snowflake value string and its column type into a serde_json::Value.
pub fn parse_snowflake_value(value: Option<&String>, column_type: &str) -> Value {
    let Some(value) = value else {
        return Value::Null; // Handle None case
    };
    match column_type.to_ascii_lowercase().as_str() {
        // Numeric types
        "fixed" | "number" | "decimal" | "numeric" | "int" | "integer" | "bigint" | "smallint"
        | "tinyint" | "byteint" => {
            if let Ok(i) = value.parse::<i64>() {
                Value::from(i)
            } else if let Ok(f) = value.parse::<f64>() {
                Value::from(f)
            } else {
                Value::String(value.to_string())
            }
        }
        "real" | "float" | "float4" | "float8" | "double" | "double precision" => {
            if let Ok(f) = value.parse::<f64>() {
                Value::from(f)
            } else {
                Value::String(value.to_string())
            }
        }
        // String & binary types
        "varchar" | "char" | "character" | "string" | "text" => Value::String(value.to_string()),
        "binary" | "varbinary" => Value::String(value.to_string()),
        // Logical
        "boolean" => match value.to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Value::Bool(true),
            "false" | "no" | "0" => Value::Bool(false),
            _ => Value::String(value.to_string()),
        },
        // Date & time types
        "date" => {
            if let Some(date) = parse_snowflake_date(value) {
                Value::String(date.format("%Y-%m-%d").to_string())
            } else {
                Value::String(value.to_string())
            }
        }
        "time" => {
            if let Some(time) = parse_snowflake_time(value, 9) {
                Value::String(time.format("%H:%M:%S%.f").to_string())
            } else {
                Value::String(value.to_string())
            }
        }
        "timestamp_ntz" | "timestamp_ltz" => {
            if let Some(dt) = parse_snowflake_timestamp_ntz_ltz(value, 9) {
                Value::String(dt.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
            } else {
                Value::String(value.to_string())
            }
        }
        "timestamp_tz" => {
            if let Some(dt) = parse_snowflake_timestamp_tz(value, 9) {
                Value::String(dt.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
            } else {
                Value::String(value.to_string())
            }
        }
        "datetime" | "timestamp" => Value::String(value.to_string()),
        // Semi-structured
        "variant" | "object" | "array" => {
            serde_json::from_str(value).unwrap_or(Value::String(value.to_string()))
        }
        // Geospatial
        "geography" | "geometry" => Value::String(value.to_string()),
        // Vector
        "vector" => serde_json::from_str(value).unwrap_or(Value::String(value.to_string())),
        // Unknown/unsupported types: preserve as string
        _ => Value::String(value.to_string()),
    }
}
