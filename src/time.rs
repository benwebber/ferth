//! System clock.
#[cfg(feature = "std")]
mod inner {
    use crate::double::Double;
    use std::sync::LazyLock;
    use std::thread;
    use std::time::{Duration, Instant};

    pub static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

    pub fn utime() -> Double {
        let d = Instant::now() - *EPOCH;
        Double::from(d.as_micros() as u64)
    }

    pub fn sleep_ms(ms: usize) {
        thread::sleep(Duration::from_millis(ms as u64));
    }
}

#[cfg(feature = "std")]
pub(crate) use inner::{EPOCH, sleep_ms, utime};

/// A UTC date and time.
#[derive(Debug, Clone, Copy, Default)]
pub struct DateTime {
    pub year: usize,
    pub month: usize,
    pub day: usize,
    pub hour: usize,
    pub minute: usize,
    pub second: usize,
}

impl DateTime {
    #[cfg(feature = "time")]
    pub(crate) fn now() -> Self {
        use chrono::{Datelike, Timelike};
        let now = chrono::Utc::now();
        Self {
            year: now.year() as usize,
            month: now.month() as usize,
            day: now.day() as usize,
            hour: now.hour() as usize,
            minute: now.minute() as usize,
            second: now.second() as usize,
        }
    }
}
