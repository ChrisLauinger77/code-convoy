pub fn duration(seconds: u64) -> String {
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// UTC formatting avoids a timezone dependency and remains useful offline on
/// every platform. Bound malformed persisted timestamps before date arithmetic.
pub fn timestamp(seconds: u64) -> String {
    if seconds >= 253_402_300_800 {
        return "Date unavailable".into();
    }
    let days = seconds / 86_400 + 719_162; // Complete Gregorian days before 1970-01-01.
    let mut year = days / 146_097 * 400 + 1;
    let mut day = days % 146_097;
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    loop {
        let length = if leap(year) { 366 } else { 365 };
        if day < length {
            break;
        }
        day -= length;
        year += 1;
    }
    let months = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for length in months {
        if day < length {
            break;
        }
        day -= length;
        month += 1;
    }
    format!(
        "{year:04}-{month:02}-{:02} {:02}:{:02}:{:02} UTC",
        day + 1,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

pub(super) fn rfc3339(seconds: u64) -> Option<String> {
    (seconds < 253_402_300_800).then(|| timestamp(seconds).replace(' ', "T").replace("TUTC", "Z"))
}
pub(super) fn display(value: &str) -> String {
    value.replace('T', " ").replace('Z', " UTC")
}
pub(super) fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "Unavailable".into(), display)
}
