#[cfg(feature = "std")]
mod inner {
    use std::sync::LazyLock;
    use std::time::Instant;

    pub static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

    macro_rules! debug {
        ($name:expr, $($arg:tt)*) => {
            if cfg!(debug_assertions) {
                eprintln!("[{:>9}] [{}] {}", crate::log::EPOCH.elapsed().as_nanos(), $name, format!($($arg)*));
            }
        }
    }

    pub(crate) use debug;
}

#[cfg(not(feature = "std"))]
mod inner {
    macro_rules! debug {
        ($($arg:tt)*) => {};
    }
    pub(crate) use debug;
}

pub(crate) use inner::debug;

#[cfg(feature = "std")]
pub use inner::EPOCH;
