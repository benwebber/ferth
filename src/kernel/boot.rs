use crate::data::{Data, Mem};
use crate::double::Double;
use crate::error::{Ior, KernelError};
use crate::io::Io;
use crate::log::debug;
use crate::vm::{Op, Vm};
use crate::{BL, Error, FALSE, Result, SIZE, TRUE};

use super::builtins::{
    compile_comma, decode, emit, find, header, key, numberq, parse, refill, to_number,
};
use super::env;
use super::host;
use super::layout;
use super::{
    BOOTSTRAP, BUILTIN, Bootstrapping, Builtin, COLON, HIDDEN, IMMEDIATE, Kernel, MAX_BUILTINS,
    PRIMITIVE, Ready,
};

use env::Environment;
use layout::Layout;

pub use env::Config;
pub use host::Host;

const KERNEL: &[u8] = include_bytes!("../kernel.fth");

#[derive(Clone, Copy)]
enum Token {
    Lit(usize),
    Name(&'static [u8]),
}

impl<M: Mem, I: Io> Kernel<M, I, Bootstrapping> {
    pub fn new(mem: M, io: I, config: Config) -> Self {
        let env = Environment {
            config,
            ..Default::default()
        };
        let data = Data::new(mem);
        let vm = Vm::new(env.config.stack_cells, env.config.return_stack_cells);
        let layout_base = vm.reserved();
        Self {
            vm,
            data,
            io,
            builtins: [None; MAX_BUILTINS],
            builtins_len: 0,
            op_xts: [0; 256],
            layout_base,
            env,
            state: Bootstrapping {},
        }
    }

    pub fn boot(mut self) -> Result<Kernel<M, I, Ready>> {
        self.reserve_variables()?;
        debug!("KERNEL", "Reserved variables");
        self.compile_opcodes()?;
        debug!("KERNEL", "Compiled opcodes");
        self.register_builtins()?;
        debug!("KERNEL", "Registered builtins");
        self.compile_environment()?;
        debug!("KERNEL", "Compiled environment");
        self.define_variables()?;
        debug!("KERNEL", "Defined variables");
        self.compile_compiler()?;
        debug!("KERNEL", "Compiled compiler");
        self.load_kernel()?;
        debug!("KERNEL", "Loaded kernel");
        let xt = |name: &'static str| -> Result<usize> {
            self.find(name.as_bytes())?
                .map(|(xt, _)| xt)
                .ok_or(KernelError::MissingEntryPoint(name).into())
        };
        let state = Ready {
            xt_catch: xt("catch")?,
            xt_interpret: xt("(interpret)")?,
        };
        Ok(Kernel {
            vm: self.vm,
            data: self.data,
            io: self.io,
            op_xts: self.op_xts,
            builtins: self.builtins,
            builtins_len: self.builtins_len,
            layout_base: self.layout_base,
            env: self.env,
            state,
        })
    }

    /// Reserve cells for system variables.
    fn reserve_variables(&mut self) -> Result<()> {
        let variables: &[(usize, usize)] = &[
            (Layout::HERE, self.layout_base + Layout::DATA),
            (Layout::LATEST, 0),
            (Layout::SOURCE_ADDR, self.layout_base + Layout::INPUT),
            (Layout::SOURCE_LEN, 0),
            (Layout::SOURCE_ID, 0),
            (Layout::TO_IN, 0),
            (Layout::BASE, 10),
            (Layout::STATE, 0),
            (Layout::SP0, Vm::DS_ADDR),
            (Layout::RP0, self.vm.rs_addr() + SIZE),
            (Layout::DIAGNOSTIC_ADDR, 0),
            (Layout::DIAGNOSTIC_LEN, 0),
        ];
        for (offset, value) in variables {
            self.data.write_cell(self.layout_base + offset, *value)?;
        }
        Ok(())
    }

    /// Compile inner interpreter words ("opcodes").
    ///
    /// The inner interpreter implements these words directly. They comprise the most fundamental
    /// set of execution, stack, memory, and arithmetic operations.
    fn compile_opcodes(&mut self) -> Result<()> {
        let opcodes: &[(&[u8], Op)] = &[
            (b"!", Op::Store),
            (b"(docreate)", Op::DoCreate),
            (b"(exit)", Op::Exit),
            (b"(jmp)", Op::Jmp),
            (b"(jmpz)", Op::JmpZ),
            (b"(lit)", Op::Lit),
            (b"(nand)", Op::Nand),
            (b"(do)", Op::Do),
            (b"(?do)", Op::QDo),
            (b"(+loop)", Op::PlusLoop),
            (b"unloop", Op::Unloop),
            (b"i", Op::I),
            (b"j", Op::J),
            (b"(s\")", Op::Str),
            (b"um*", Op::UmMul),
            (b"+", Op::Add),
            (b"0<", Op::LtZ),
            (b"0=", Op::EqZ),
            (b">r", Op::ToR),
            (b"@", Op::Fetch),
            (b"c!", Op::CStore),
            (b"c@", Op::CFetch),
            (b"(sp@)", Op::SpFetch),
            (b"(sp!)", Op::SpStore),
            (b"(rp@)", Op::RpFetch),
            (b"(rp!)", Op::RpStore),
            (b"drop", Op::Drop),
            (b"dup", Op::Dup),
            (b"r>", Op::RFrom),
            (b"swap", Op::Swap),
            (b"lshift", Op::LShift),
            (b"rshift", Op::RShift),
            (b"um/mod", Op::UmDivMod),
            (b"execute", Op::Execute),
            (b"(call)", Op::Call),
            (b"(yield)", Op::Yield),
        ];
        for (name, op) in opcodes {
            let xt = self.define(name, *op, PRIMITIVE)?;
            self.comma(Op::Exit as usize)?;
            self.op_xts[*op as usize] = xt;
        }
        Ok(())
    }

    /// Compile outer interpreter words ("builtins").
    ///
    /// These words concern parsing and I/O. They may exist as builtins for several reasons. The
    /// parsing words are difficult, or inefficient, to express in Forth. The inner interpreter
    /// lacks any I/O facilities, so the outer interpreter naturally has to provide these.
    fn register_builtins(&mut self) -> Result<()> {
        let builtins: &[(&[u8], Builtin, u8)] = &[
            (b"emit", emit, 0),
            (b"(find)", find, 0),
            (b"key", key, 0),
            (b"parse", parse, 0),
            (b"refill", refill, 0),
            (b"(header)", header, 0),
            (b">number", to_number, 0),
            (b"(number?)", numberq, 0),
            (b"compile,", compile_comma, 0),
            (b"(decode)", decode, 0),
        ];
        for (name, f, flags) in builtins {
            self.register_builtin(name, *f, *flags)?;
        }
        Ok(())
    }

    /// Compile compiler words.
    #[rustfmt::skip]
    fn compile_compiler(&mut self) -> Result<()> {
        macro_rules! compile {
            ($s:expr, $flags:expr, [$($body:expr),* $(,)?]) => {
                self.compile($s, $flags, &[$($body),*])?;
            };
        }

        macro_rules! addr {
            ($name:ident) => {
                L(self.layout_addr(Layout::$name))
            };
        }

        use Token::{Lit as L, Name as N};

        // This sequence hand-compiles the words `:`, `;`, `literal`, and their direct
        // dependencies. This code *is* Forth, just not written as text.
        //
        // `N(name)` compiles a call to a previously defined word. Any reference to an XT that
        // should be a data value at runtime (`['] word`) must be a literal (`L(xt)`).

        // cells
        compile!(b"cells", 0, [L(SIZE), N(b"um*"), N(b"drop")]);

        // : +! ( u addr -- ) dup >r @ + r> ! ;
        compile!(b"+!", 0, [N(b"dup"), N(b">r"), N(b"@"), N(b"+"), N(b"r>"), N(b"!")]);

        // : allot ( n -- ) (here) +! ;
        compile!(b"allot", 0, [addr!(HERE), N(b"+!")]);

        // : , ( x -- ) here ! 1 cells allot ;
        //
        // `here` is always cell-aligned at this stage so it is safe call `,` without `align`.
        compile!(
            b",",
            0,
            [addr!(HERE), N(b"@"), N(b"!"), L(1), N(b"cells"), N(b"allot")]
        );

        // : literal ( x -- ) ['] (lit) , , ; immediate
        //
        // The first lit is the (lit) opcode. The second lit is the XT of (lit).
        // At runtime this executes as `(lit) xt`, compiling the XT of (lit) and then the
        // original top of stack.
        compile!(
            b"literal",
            IMMEDIATE,
            [L(Op::Lit as usize), N(b","), N(b",")]
        );

        // : ['] bl parse (find) drop literal ; immediate
        //
        // This definition does not check the flag value because bootstrap input is trusted.
        // `literal` is immediate, so in Forth, it would require `postpone literal` to compile into
        // this definition. Here we can compile it directly.
        compile!(
            b"[']",
            IMMEDIATE | BOOTSTRAP,
            [L(BL), N(b"parse"), N(b"(find)"), N(b"drop"), N(b"literal")]
        );

        // : :
        //   bl parse (header)
        //   -1 state !
        // ;
        //
        // Parse a word and create a definition for it. This simple definition does not set the
        // hidden flag. The Forth kernel replaces it.
        compile!(
            b":",
            BOOTSTRAP,
            [
                L(BL), N(b"parse"), N(b"(header)"),
                L(TRUE), N(b"state"), N(b"!"),
            ]
        );

        // : ;
        //   ['] (exit) compile,
        //   \ Calculate and set bodylen.
        //   (latest) @                 ( xt )
        //   dup 3 cells -              ( xt bodylen-addr )
        //   swap                       ( bodylen-addr xt )
        //   here swap -                ( bodylen-addr bodylen )
        //   swap !                     ( )
        //   \ Unset hidden flag.
        //   (latest) @                 ( latest )
        //   (flags-addr) dup c@        ( flags-addr flags )
        //   (hidden-flag) invert and   ( flags-addr flags' )
        //   swap c!
        //   0 state !
        // ; immediate
        //
        // Compile a literal to compile `exit`, store the bodylen, unset the hidden flag, then exit
        // compilation mode.
        //
        // Unlike `:`, this definition must be functionally complete. It must unset the hidden flag
        // and set bodylen. We cannot define a simpler version of `;` here and redefine a more
        // complete version in in Forth because this `;` would terminate that definition.
        //
        // It should be possible, however, to implement an optimizing `;` in Forth that compiles
        // jumps for tail calls.
        compile!(
            b";",
            IMMEDIATE,
            [
                L(Op::Exit as usize), N(b","),
                // Calculate and set bodylen.
                addr!(LATEST), N(b"@"),
                N(b"dup"), L((3 * SIZE).wrapping_neg()), N(b"+"),
                N(b"swap"),
                addr!(HERE), N(b"@"), N(b"swap"),
                N(b"dup"), N(b"(nand)"), L(1), N(b"+"), N(b"+"), // inline -
                N(b"swap"), N(b"!"),
                // Unset hidden flag.
                addr!(LATEST), N(b"@"),
                L((2 * SIZE).wrapping_neg()), N(b"+"), L(1), N(b"+"), N(b"dup"), N(b"c@"),
                L(HIDDEN.into()), N(b"dup"), N(b"(nand)"), N(b"(nand)"), N(b"dup"), N(b"(nand)"), // inline invert, and
                N(b"swap"), N(b"c!"),
                L(FALSE), N(b"state"), N(b"!"),
            ]
        );
        Ok(())
    }

    fn load_kernel(&mut self) -> Result<()> {
        for line in KERNEL.split(|&b| b == b'\n') {
            if !line.is_empty() {
                self.set_source(line)?;
                self.interpret()?;
            }
        }
        Ok(())
    }

    fn compile_environment(&mut self) -> Result<()> {
        let flag = |b: bool| -> usize { if b { TRUE } else { FALSE } };
        self.compile(
            b"(/counted-string)",
            0,
            &[Token::Lit(self.env.counted_string)],
        )?;
        self.compile(b"(/hold)", 0, &[Token::Lit(self.env.config.hold)])?;
        self.compile(b"(/pad)", 0, &[Token::Lit(self.env.config.pad)])?;
        self.compile(
            b"(address-unit-bits)",
            0,
            &[Token::Lit(self.env.address_unit_bits)],
        )?;
        self.compile(b"(floored)", 0, &[Token::Lit(flag(self.env.floored))])?;
        self.compile(b"(max-char)", 0, &[Token::Lit(self.env.max_char)])?;
        let (lo, hi): (usize, usize) = Double(self.env.max_d.0 as _).into();
        self.compile(b"(max-d)", 0, &[Token::Lit(lo), Token::Lit(hi)])?;
        self.compile(b"(max-n)", 0, &[Token::Lit(self.env.max_n as usize)])?;
        self.compile(b"(max-u)", 0, &[Token::Lit(self.env.max_u)])?;
        let (lo, hi): (usize, usize) = self.env.max_ud.into();
        self.compile(b"(max-ud)", 0, &[Token::Lit(lo), Token::Lit(hi)])?;
        self.compile(
            b"(return-stack-cells)",
            0,
            &[Token::Lit(self.env.config.return_stack_cells)],
        )?;
        self.compile(
            b"(stack-cells)",
            0,
            &[Token::Lit(self.env.config.stack_cells)],
        )?;
        Ok(())
    }

    /// Define variables for the system variable addresses.
    fn define_variables(&mut self) -> Result<()> {
        let variables: &[(&[u8], usize)] = &[
            (b"(here)", Layout::HERE),
            (b"(latest)", Layout::LATEST),
            (b"(source-addr)", Layout::SOURCE_ADDR),
            (b"(source-len)", Layout::SOURCE_LEN),
            (b"(source-id)", Layout::SOURCE_ID),
            (b">in", Layout::TO_IN),
            (b"base", Layout::BASE),
            (b"state", Layout::STATE),
            (b"(sp0)", Layout::SP0),
            (b"(rp0)", Layout::RP0),
            (b"(diagnostic-addr)", Layout::DIAGNOSTIC_ADDR),
            (b"(diagnostic-len)", Layout::DIAGNOSTIC_LEN),
        ];
        for (name, offset) in variables {
            self.compile(name, 0, &[Token::Lit(self.layout_base + offset)])?;
        }
        Ok(())
    }

    fn compile(&mut self, name: &[u8], flags: u8, body: &[Token]) -> Result<usize> {
        let xt = self.create(name, flags | COLON)?;
        self.data.write_cell(self.layout_addr(Layout::LATEST), xt)?;
        self.data.write_cell(self.layout_addr(Layout::HERE), xt)?;
        for &token in body {
            match token {
                Token::Lit(x) => {
                    self.comma(Op::Lit as usize)?;
                    self.comma(x)?;
                }
                Token::Name(name) => {
                    let xt = self
                        .find(name)?
                        .map(|(xt, _)| xt)
                        .ok_or(Error::Throw(Ior::UNDEFINED_WORD))?;
                    self.push(xt)?;
                    compile_comma(self)?;
                }
            }
        }
        self.comma(Op::Exit as usize)?;
        let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
        self.data.write_cell(xt - 3 * SIZE, here - xt)?;
        Ok(xt)
    }

    fn comma(&mut self, val: usize) -> Result<()> {
        let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
        self.data.write_cell(here, val)?;
        self.data
            .write_cell(self.layout_addr(Layout::HERE), here + SIZE)?;
        Ok(())
    }

    /// Parse the next token in the parse area, skipping leading whitespace.
    ///
    /// ```text
    /// parse-name ( "<spaces>name<space>" -- c-addr u )
    /// ```
    ///
    /// See [`PARSE-NAME`](https://forth-standard.org/standard/core/PARSE-NAME).
    // TODO: Thread this through the stack like the other words.
    fn parse_name(&mut self) -> Result<(usize, usize)> {
        let src = self.data.read_cell(self.layout_addr(Layout::SOURCE_ADDR))?;
        let src_len = self.data.read_cell(self.layout_addr(Layout::SOURCE_LEN))?;
        let mut to_in = self.data.read_cell(self.layout_addr(Layout::TO_IN))?;
        while to_in < src_len && self.data.read_char(src + to_in)?.is_ascii_whitespace() {
            to_in += 1;
        }
        self.data
            .write_cell(self.layout_addr(Layout::TO_IN), to_in)?;
        self.push(BL)?;
        parse(self)?;
        let len = self.pop()?;
        let addr = self.pop()?;
        Ok((addr, len))
    }

    /// The main interpreter loop.
    ///
    /// <https://forth-standard.org/standard/usage#section.3.4>
    fn interpret(&mut self) -> Result<()> {
        loop {
            let (addr, len) = self.parse_name()?;
            if len == 0 {
                return Ok(());
            }
            self.push(addr)?;
            self.push(len)?;
            find(self)?;
            let flag = self.pop()? as isize;
            let state = self.data.read_cell(self.layout_addr(Layout::STATE))?;
            if flag != 0 {
                if state == 0 || flag == 1 {
                    let xt = self.pop()?;
                    self.execute(xt)?;
                } else {
                    compile_comma(self)?;
                }
            } else {
                self.push(addr)?;
                self.push(len)?;
                numberq(self)?;
                let ok = self.pop()? as isize;
                if ok == 1 {
                    let v = self.pop()?;
                    if state != 0 {
                        self.comma(Op::Lit as usize)?;
                        self.comma(v)?;
                    } else {
                        self.push(v)?;
                    }
                } else {
                    // numberq leaves ( c-addr u ) on failure; discard and report the word.
                    self.pop()?;
                    self.pop()?;
                    return self.undefined(addr, len);
                }
            }
        }
    }

    fn register_builtin(&mut self, name: &[u8], f: Builtin, flags: u8) -> Result<()> {
        let idx = self.builtins_len;
        if idx >= MAX_BUILTINS {
            return Err(KernelError::BuiltinTableFull.into());
        }
        self.builtins[idx] = Some(f);
        self.builtins_len += 1;
        self.define(name, Op::Yield, flags | BUILTIN)?;
        self.comma(idx)?;
        self.comma(Op::Exit as usize)
    }

    fn define(&mut self, name: &[u8], code: Op, flags: u8) -> Result<usize> {
        let kind = match code {
            Op::Yield => BUILTIN,
            _ => PRIMITIVE,
        };
        let cfa = self.create(name, flags | kind)?;
        self.data.write_cell(cfa, code as usize)?;
        self.data
            .write_cell(self.layout_addr(Layout::HERE), cfa + SIZE)?;
        self.data
            .write_cell(self.layout_addr(Layout::LATEST), cfa)?;
        Ok(cfa)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::NoIo;
    use crate::kernel::{BUILTIN, COLON, PRIMITIVE};

    #[test]
    fn tag_boot_words_with_kind() {
        let k = Kernel::new([0u8; 65536], NoIo, Config::default())
            .boot()
            .unwrap();
        let kind = |name: &[u8]| {
            let (xt, _) = k.find(name).unwrap().unwrap();
            (k.data.read_cell(xt - super::super::INFO_FROM_CFA).unwrap() >> 8) as u8
        };
        assert!(kind(b"dup") & PRIMITIVE != 0);
        assert!(kind(b"(find)") & BUILTIN != 0);
        assert!(kind(b"cells") & COLON != 0);
    }
}
