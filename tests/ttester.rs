use ferth::io::BufIo;
use ferth::{Error, Fe};

struct TtesterError {
    error: Error,
    output: String,
}

impl std::fmt::Debug for TtesterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}", self.error, self.output)
    }
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
            // BufIo errors when output buffer fills, so it must be large enough to hold all test
            // output.
            let mut dest: Vec<u8> = vec![0u8; 1 << 16];
            // `&dest` used below
            let result = (|| -> ferth::Result<()> {
                let io = BufIo::new(&src, &mut dest);
                let mut fe = Fe::new(vec![0u8; 1 << 17], io)?;
                fe.load()
            })();
            result.map_err(|error| {
                let end = dest.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                let output = String::from_utf8_lossy(&dest[..end]).into_owned();
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
