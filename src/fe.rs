use crate::counted::CountedStr31;
use crate::data::Mem;
use crate::io::{Io, NoIo};
use crate::{Error, Result};

mod kernel;

use kernel::{Environment, Kernel};

pub struct Fe<M: Mem = [u8; 65536], I: Io = NoIo> {
    kernel: Kernel<M, I>,
}

impl<M: Mem, I: Io> Fe<M, I> {
    pub fn new(mem: M, io: I) -> Result<Self> {
        Ok(Self {
            kernel: Kernel::new(mem, io)?,
        })
    }
    pub fn with_env(mem: M, io: I, env: Environment) -> Result<Self> {
        Ok(Self {
            kernel: Kernel::with_env(mem, io, env)?,
        })
    }

    pub fn evaluate(&mut self, code: &[u8]) -> Result<()> {
        self.kernel.set_source(code)?;
        let s = CountedStr31::try_from(b"(interpret)".as_slice())?;
        let (xt, _) = self
            .kernel
            .lookup(b"(interpret)")?
            .ok_or(Error::UndefinedWord(s))?;
        self.kernel.run(xt)
    }

    pub fn push(&mut self, x: usize) -> Result<()> {
        self.kernel.push(x)
    }
    pub fn pop(&mut self) -> Result<usize> {
        self.kernel.pop()
    }
    pub fn reset(&mut self) {
        self.kernel.reset()
    }
    pub fn stack(&self) -> impl Iterator<Item = usize> + '_ {
        self.kernel.stack()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FALSE, TRUE};

    type TestFe = Fe;

    #[test]
    fn test_undefined_word() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        assert!(matches!(fe.evaluate(b"nope"), Err(Error::UndefinedWord(_))));
    }

    #[test]
    fn test_long_word_name_errors() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        // A name at the limit (30 bytes) is accepted.
        let ok = [b": ".as_slice(), &[b'a'; 30], b" 1 ;"].concat();
        assert!(fe.evaluate(&ok).is_ok());
        // A name that is too long returns an error instead of panicking.
        let long = [b": ".as_slice(), &[b'a'; 40], b" 1 ;"].concat();
        assert_eq!(fe.evaluate(&long), Err(Error::CountedStrTooLong(40)));
    }

    #[test]
    fn test_environment() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();

        let single = |fe: &mut TestFe, q: &[u8], expected: usize| {
            fe.evaluate(q).unwrap();
            assert_eq!(fe.pop().unwrap(), TRUE);
            assert_eq!(fe.pop().unwrap(), expected);
        };
        let double = |fe: &mut TestFe, q: &[u8], lo: usize, hi: usize| {
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
        single(&mut fe, br#"s" /HOLD" environment?"#, 64);
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
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();

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
}
