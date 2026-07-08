use ferth::double::Double;
use ferth::host::{Clock, Io};
use ferth::time::DateTime;
use ferth::{Error, Ferth, Result};

struct TtesterError {
    error: Error,
    output: String,
}

impl std::fmt::Debug for TtesterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}", self.error, self.output)
    }
}

/// A host implementation that reads from any `BufRead` and writes to any `Write`.
///
/// Does not keep time.
pub struct TtesterHost<R, W> {
    input: R,
    output: W,
}

impl<R, W> TtesterHost<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Self { input, output }
    }
}

impl<R: ::std::io::BufRead, W: ::std::io::Write> Io for TtesterHost<R, W> {
    fn key(&mut self) -> Result<Option<u8>> {
        let mut b = [0u8; 1];
        match self.input.read(&mut b) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(b[0])),
            Err(_) => Err(Error::Io),
        }
    }

    fn emit(&mut self, u: u8) -> Result<()> {
        self.output.write_all(&[u]).map_err(|_| Error::Io)
    }

    fn refill(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        let mut line = Vec::new();
        let n = self
            .input
            .read_until(b'\n', &mut line)
            .map_err(|_| Error::Io)?;
        if n == 0 {
            return Ok(None);
        }
        // Strip end of line characters.
        if line.last() == Some(&b'\n') {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
        }
        let len = line.len().min(buf.len());
        buf[..len].copy_from_slice(&line[..len]);
        Ok(Some(len))
    }
}

impl<R, W> Clock for TtesterHost<R, W> {
    fn utime(&self) -> Double {
        Double::default()
    }

    fn time_and_date(&self) -> DateTime {
        DateTime::default()
    }

    fn sleep_ms(&self, _ms: usize) {}
}

macro_rules! ttester {
    ($name:ident $(, $filename:expr)*) => {
        #[test]
        fn $name() -> std::result::Result<(), TtesterError> {
            let src: Vec<u8> = [
                include_bytes!("ttester-shim.fth") as &[u8],
                include_bytes!("forth2012-test-suite/src/ttester.fs"),
                // ttester "vectors" errors, or redirects them, to ERROR-XT.
                // The user can customize how the test harness handles errors by setting ERROR-XT
                // to a custom handler. Here the handler:
                //   1. raises a DivisionByZero to fail the test, and
                //   2. calls the test suite's ERROR1, which prints the error.
                b": ERROR-THROW ERROR1 1 0 / ; ' ERROR-THROW ERROR-XT !\n",
                $(include_bytes!($filename) as &[u8],)*
            ]
            .concat();
            let mut dest: Vec<u8> = Vec::new();
            // `&dest` used below
            let result = (|| -> ferth::Result<()> {
                let io = TtesterHost::new(src.as_slice(), &mut dest);
                let mut fe = Ferth::new(vec![0u8; 1 << 17], io)?;
                fe.load()
            })();
            result.map_err(|error| {
                let output = String::from_utf8_lossy(&dest).into_owned();
                TtesterError { error, output }
            })
        }
    }
}

ttester!(test_load_test_harness);
ttester!(
    test_core,
    "forth2012-test-suite/src/core.fr",
    "forth2012-test-suite/src/coreplustest.fth"
);
ttester!(
    test_coreext,
    "forth2012-test-suite/src/errorreport.fth",
    "forth2012-test-suite/src/utilities.fth",
    "forth2012-test-suite/src/coreexttest.fth"
);
