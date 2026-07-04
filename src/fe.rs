use crate::Result;
use crate::data::Mem;
use crate::error::{Error, KernelError};
use crate::io::{Io, NoIo};
use crate::kernel::{Config, Kernel};
use crate::log::debug;
use crate::state::{Booted, Loading, Ready, State};

const WORDLISTS: &[(&str, &[u8])] = &[
    ("core", include_bytes!("core.fth")),
    ("core-ext", include_bytes!("core-ext.fth")),
    ("tools", include_bytes!("tools.fth")),
];

/// The Forth system.
pub struct Fe<M: Mem = [u8; 65536], I: Io = NoIo, S: State = Ready> {
    kernel: Kernel<M, I, Booted>,
    state: S,
}

impl<M: Mem, I: Io, S: State> Fe<M, I, S> {
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

impl<M: Mem, I: Io> Fe<M, I, Loading> {
    /// Build an [`Fe`] with the default environment configuration.
    pub fn new(mem: M, io: I) -> Result<Fe<M, I, Ready>> {
        Self::with_config(mem, io, Config::default())
    }

    /// Build an [`Fe`] with a specific environment configuration.
    pub fn with_config(mem: M, io: I, config: Config) -> Result<Fe<M, I, Ready>> {
        let mut fe = Fe {
            kernel: Kernel::new(mem, io, config)?.boot()?,
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
        Ok(Fe {
            kernel: fe.kernel,
            state,
        })
    }
}

impl<M: Mem, I: Io> Fe<M, I, Ready> {
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
        let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();
        assert!(matches!(
            fe.evaluate(b"nope"),
            Err(Error::Throw(Ior::UNDEFINED_WORD))
        ));
    }

    #[test]
    fn test_long_word_name_errors() {
        let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();
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
        let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();

        let single = |fe: &mut Fe, q: &[u8], expected: usize| {
            fe.evaluate(q).unwrap();
            assert_eq!(fe.pop().unwrap(), TRUE);
            assert_eq!(fe.pop().unwrap(), expected);
        };
        let double = |fe: &mut Fe, q: &[u8], lo: usize, hi: usize| {
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
        let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();

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
        let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();

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
        let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();

        // Re-raise recoverable errors (division by zero) as a Forth exception. `catch` returns its
        // *ior*.
        fe.evaluate(b": divzero 0 0 0 um/mod ;").unwrap();
        fe.evaluate(b"' divzero catch").unwrap();
        assert_eq!(fe.pop().unwrap() as isize, Ior::DIVISION_BY_ZERO.into());
    }
}
