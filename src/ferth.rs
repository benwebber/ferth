use crate::Result;
use crate::data::Mem;
use crate::error::{Error, KernelError};
use crate::host::{Io, MaybeClock, NullHost};
use crate::kernel::{Config, Kernel};
use crate::log::debug;
use crate::state::{Booted, Loading, Ready, State};

const WORDLISTS: &[(&str, &[u8])] = &[
    ("core", include_bytes!("core.fth")),
    ("core-ext", include_bytes!("core-ext.fth")),
    ("tools", include_bytes!("tools.fth")),
];

/// The Forth system.
pub struct Ferth<M: Mem = [u8; 65536], H: Io = NullHost, S: State = Ready> {
    kernel: Kernel<M, H, Booted>,
    state: S,
}

impl<M: Mem, H: Io, S: State> Ferth<M, H, S> {
    /// Evaluate Forth code.
    pub fn evaluate(&mut self, code: impl AsRef<[u8]>) -> Result<()> {
        for line in code.as_ref().split(|&u| u == b'\n') {
            self.kernel.set_source(line)?;
            self.kernel.catch_interpret()?;
        }
        Ok(())
    }

    /// Push a value onto the data stack.
    pub fn push(&mut self, x: usize) -> Result<()> {
        self.kernel.push(x)
    }

    /// Pop a value from the data stack.
    pub fn pop(&mut self) -> Result<usize> {
        self.kernel.pop()
    }

    /// Reset the data and return stacks.
    pub fn reset(&mut self) {
        self.kernel.reset()
    }

    /// Iterate over the data stack.
    pub fn stack(&self) -> impl Iterator<Item = usize> + '_ {
        self.kernel.stack()
    }
}

impl<M: Mem, H: Io> Ferth<M, H, Loading> {
    /// Build a [`Ferth`] with the default environment configuration.
    pub fn new(mem: M, host: H) -> Result<Ferth<M, H, Ready>>
    where
        H: MaybeClock,
    {
        Self::with_config(mem, host, Config::default())
    }

    /// Build a [`Ferth`] with a specific environment configuration.
    pub fn with_config(mem: M, host: H, config: Config) -> Result<Ferth<M, H, Ready>>
    where
        H: MaybeClock,
    {
        let mut fe = Ferth {
            kernel: Kernel::new(mem, host, config)?.boot()?,
            state: Loading {},
        };
        #[allow(unused_variables)] // name in debug!
        for (name, src) in WORDLISTS {
            fe.evaluate(src)?;
            debug!("SYSTEM", "Loaded {} wordlist", name);
        }
        let mut xt = |name: &'static str| -> Result<usize> {
            fe.kernel
                .dict()
                .find(name.as_bytes())?
                .map(|(xt, _)| xt)
                .ok_or(KernelError::MissingEntryPoint(name).into())
        };
        let state = Ready {
            xt_load: xt("(load)")?,
            xt_quit: xt("quit")?,
        };
        fe.evaluate(b"(check-bootstrap)")?;
        debug!("SYSTEM", "Passed boot checks");
        debug!("SYSTEM", "Ready");
        Ok(Ferth {
            kernel: fe.kernel,
            state,
        })
    }
}

impl<M: Mem, H: Io> Ferth<M, H, Ready> {
    /// Load and interpret code from the current input source.
    pub fn load(&mut self) -> Result<()> {
        // unwrap: Safe because typestate validates kernel already defined `catch`.
        // TODO: add boot XTs to Ready state.
        let catch_xt = self
            .kernel
            .dict()
            .find(b"catch")
            .unwrap()
            .map(|(xt, _)| xt)
            .unwrap();
        self.push(self.state.xt_load)?;
        self.kernel.execute(catch_xt)?;
        let code = self.pop()? as isize;
        if code != 0 {
            return Err(Error::Throw(code.into()));
        }
        Ok(())
    }

    /// Run `quit`, the Forth interpreter loop.
    ///
    /// See [`QUIT`](https://forth-standard.org/standard/core/QUIT).
    pub fn quit(&mut self) -> Result<()> {
        self.kernel.execute(self.state.xt_quit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{Error, Ior};
    use crate::{FALSE, TRUE};

    #[test]
    fn test_undefined_word() {
        let mut fe = Ferth::new([0u8; 65536], NullHost).unwrap();
        assert!(matches!(
            fe.evaluate(b"nope"),
            Err(Error::Throw(Ior::UNDEFINED_WORD))
        ));
    }

    #[test]
    fn test_long_word_name_errors() {
        let mut fe = Ferth::new([0u8; 65536], NullHost).unwrap();
        // A name at the limit (30 bytes) is accepted.
        let ok = [b": ".as_slice(), &[b'a'; 30], b" 1 ;"].concat();
        assert!(fe.evaluate(&ok).is_ok());
        // A name that is too long returns an error instead of panicking.
        let long = [b": ".as_slice(), &[b'a'; 40], b" 1 ;"].concat();
        assert_eq!(
            fe.evaluate(&long),
            Err(Error::Throw(Ior::DEFINITION_NAME_TOO_LONG))
        );
    }

    #[test]
    fn test_environment() {
        let mut fe = Ferth::new([0u8; 65536], NullHost).unwrap();

        let single = |fe: &mut Ferth, q: &[u8], expected: usize| {
            fe.evaluate(q).unwrap();
            assert_eq!(fe.pop().unwrap(), TRUE);
            assert_eq!(fe.pop().unwrap(), expected);
        };
        let double = |fe: &mut Ferth, q: &[u8], lo: usize, hi: usize| {
            fe.evaluate(q).unwrap();
            assert_eq!(fe.pop().unwrap(), TRUE);
            assert_eq!(fe.pop().unwrap(), hi);
            assert_eq!(fe.pop().unwrap(), lo);
        };

        single(
            &mut fe,
            br#"s" /COUNTED-STRING" environment?"#,
            u8::MAX as usize,
        );
        single(
            &mut fe,
            br#"s" /HOLD" environment?"#,
            2 * (usize::BITS as usize) + 2,
        );
        single(&mut fe, br#"s" /PAD" environment?"#, 84);
        single(
            &mut fe,
            br#"s" ADDRESS-UNIT-BITS" environment?"#,
            u8::BITS as usize,
        );
        single(&mut fe, br#"s" FLOORED" environment?"#, FALSE);
        single(&mut fe, br#"s" MAX-CHAR" environment?"#, u8::MAX as usize);
        double(
            &mut fe,
            br#"s" MAX-D" environment?"#,
            usize::MAX,
            isize::MAX as usize,
        );
        single(&mut fe, br#"s" MAX-N" environment?"#, isize::MAX as usize);
        single(&mut fe, br#"s" MAX-U" environment?"#, usize::MAX);
        double(
            &mut fe,
            br#"s" MAX-UD" environment?"#,
            usize::MAX,
            usize::MAX,
        );
        single(&mut fe, br#"s" RETURN-STACK-CELLS" environment?"#, 64);
        single(&mut fe, br#"s" STACK-CELLS" environment?"#, 64);

        fe.evaluate(br#"s" UNKNOWN" environment?"#).unwrap();
        assert_eq!(fe.pop().unwrap(), FALSE);
    }

    #[test]
    fn test_catch_throw() {
        let mut fe = Ferth::new([0u8; 65536], NullHost).unwrap();

        // Success. `catch` returns 0 and the protected word is next on stack.
        fe.evaluate(b": ok 42 ;").unwrap();
        fe.evaluate(b"' ok catch").unwrap();
        assert_eq!(fe.pop().unwrap(), 0);
        assert_eq!(fe.pop().unwrap(), 42);

        // Success. `0 throw` does not raise exception.
        fe.evaluate(b": fine 7 0 throw ;").unwrap();
        fe.evaluate(b"' fine catch").unwrap();
        assert_eq!(fe.pop().unwrap(), 0);
        assert_eq!(fe.pop().unwrap(), 7);

        // Failure. On `throw`, `catch` returns the thrown code.
        fe.evaluate(b": foo -1 throw ;").unwrap();
        fe.evaluate(b"' foo catch").unwrap();
        assert_eq!(fe.pop().unwrap(), -1isize as usize);

        // Failure. Restore data stack to the depth it was before `catch`.
        fe.evaluate(b": junk 1 2 3 42 throw ;").unwrap();
        fe.evaluate(b"47 ' junk catch").unwrap();
        assert_eq!(fe.pop().unwrap(), 42);
        assert_eq!(fe.pop().unwrap(), 47);
    }

    #[test]
    fn test_abort_irrecoverable_error() {
        use crate::error::VmError;
        let mut fe = Ferth::new([0u8; 65536], NullHost).unwrap();

        // Force a data stack overflow, an irrecoverable error.
        fe.evaluate(b": overflow begin 1 again ;").unwrap();
        assert!(matches!(
            fe.evaluate(b"' overflow catch"),
            Err(Error::Vm(VmError::StackOverflow))
        ));

        // The abort reset the machine, so the system is usable again.
        fe.evaluate(b"1 2 +").unwrap();
        assert_eq!(fe.pop().unwrap(), 3);
    }

    #[test]
    fn test_catch_recoverable_error() {
        let mut fe = Ferth::new([0u8; 65536], NullHost).unwrap();

        // Re-raise recoverable errors (division by zero) as a Forth exception. `catch` returns its
        // *ior*.
        fe.evaluate(b": divzero 0 0 0 um/mod ;").unwrap();
        fe.evaluate(b"' divzero catch").unwrap();
        assert_eq!(fe.pop().unwrap() as isize, Ior::DIVISION_BY_ZERO.into());
    }

    #[cfg(feature = "time")]
    #[test]
    fn test_time_and_date() {
        let mut fe = Ferth::new([0u8; 65536], crate::host::StdHost).unwrap();

        fe.evaluate(b"time&date").unwrap();
        let year = fe.pop().unwrap();
        let month = fe.pop().unwrap();
        let day = fe.pop().unwrap();
        let hour = fe.pop().unwrap();
        let minute = fe.pop().unwrap();
        let second = fe.pop().unwrap();

        assert!(second <= 60); // 60 to allow for a leap second
        assert!(minute <= 59);
        assert!(hour <= 23);
        assert!((1..=31).contains(&day));
        assert!((1..=12).contains(&month));
        assert!(year >= 2024);
    }

    #[cfg(feature = "time")]
    #[test]
    fn test_ms() {
        use std::time::{Duration, Instant};

        let mut fe = Ferth::new([0u8; 65536], crate::host::StdHost).unwrap();

        let start = Instant::now();
        fe.evaluate(b"10 ms").unwrap();
        assert!(start.elapsed() >= Duration::from_millis(10));
    }

    #[cfg(feature = "time")]
    #[test]
    fn test_utime() {
        use std::thread;
        use std::time::Duration;

        let mut fe = Ferth::new([0u8; 65536], crate::host::StdHost).unwrap();

        let read = |fe: &mut Ferth<[u8; 65536], crate::host::StdHost>| -> u128 {
            fe.evaluate(b"(utime)").unwrap();
            let hi = fe.pop().unwrap() as u128;
            let lo = fe.pop().unwrap() as u128;
            (hi << usize::BITS) | lo
        };

        let before = read(&mut fe);
        thread::sleep(Duration::from_millis(5));
        let after = read(&mut fe);

        assert!(after - before >= 5_000); // at least the time slept
    }
}
