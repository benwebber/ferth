use crate::data::{Data, Mem};
use crate::double::Double;
use crate::error::{Ior, KernelError};
use crate::header::{Flags, Header};
use crate::io::Io;
use crate::log::debug;
use crate::state::{Booted, Booting};
use crate::vm::{Op, Vm};
use crate::{BL, Error, FALSE, Result, SIZE, TRUE};

use super::builtins::{emit, find, header, key, refill};
use super::env;
use super::layout;
use super::{Builtin, Kernel, MAX_BUILTINS};
use crate::packed::PackedInstr;

use env::Environment;
use layout::Layout;

pub use env::Config;

const KERNEL: &[u8] = include_bytes!("../kernel.fth");

#[derive(Clone, Copy)]
enum Token {
    Lit(usize),
    Name(&'static [u8]),
    If,
    Else,
    Then,
    Begin,
    While,
    Repeat,
}

impl<M: Mem, I: Io> Kernel<M, I, Booting> {
    pub fn new(mem: M, io: I, config: Config) -> Self {
        let env = Environment {
            config,
            ..Default::default()
        };
        let data = Data::new(mem);
        let vm = Vm::new(env.config.stack_cells, env.config.return_stack_cells);
        assert!(vm.reserved() <= data.size(), "data space too small for VM");
        let layout_base = vm.reserved();
        Self {
            vm,
            data,
            io,
            builtins: [None; MAX_BUILTINS],
            builtins_len: 0,
            layout_base,
            env,
            state: Booting {},
        }
    }

    pub fn boot(mut self) -> Result<Kernel<M, I, Booted>> {
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
        self.compile_wrappers()?;
        debug!("KERNEL", "Compiled wrappers");
        self.compile_compiler()?;
        debug!("KERNEL", "Compiled compiler");
        self.load_kernel()?;
        debug!("KERNEL", "Loaded kernel");
        let mut xt = |name: &'static str| -> Result<usize> {
            self.dict()
                .find(name.as_bytes())?
                .map(|(xt, _)| xt)
                .ok_or(KernelError::MissingEntryPoint(name).into())
        };
        let state = Booted {
            xt_catch: xt("catch")?,
            xt_interpret: xt("(interpret)")?,
            xt_throw: xt("throw")?,
        };
        Ok(Kernel {
            vm: self.vm,
            data: self.data,
            io: self.io,
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
            (b"(parse)", Op::Parse),
            (b"(number)", Op::Number),
            (b"(>number)", Op::ToNumber),
            (b"(compile,)", Op::CompileComma),
            (b"(decode)", Op::Decode),
        ];
        for (name, op) in opcodes {
            let xt = self.define(name, Flags::PRIMITIVE)?;
            let instr = PackedInstr::new(*op, xt, 0)?;
            self.data.write_cell(xt, instr.into())?;
        }
        Ok(())
    }

    /// Compile outer interpreter words ("builtins").
    ///
    /// These words concern parsing and I/O. They may exist as builtins for several reasons. The
    /// parsing words are difficult, or inefficient, to express in Forth. The inner interpreter
    /// lacks any I/O facilities, so the outer interpreter naturally has to provide these.
    fn register_builtins(&mut self) -> Result<()> {
        let builtins: &[(&[u8], Builtin<M, I>, Flags)] = &[
            (b"emit", emit, Flags::EMPTY),
            (b"(find)", find, Flags::EMPTY),
            (b"key", key, Flags::EMPTY),
            (b"refill", refill, Flags::EMPTY),
            (b"(header)", header, Flags::EMPTY),
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
                self.compile($s, $flags.into(), &[$($body),*])?;
            };
        }

        macro_rules! addr {
            ($name:ident) => {
                L(self.dict().addr(Layout::$name))
            };
        }

        use Token::{Lit as L, Name as N, *};

        // This sequence hand-compiles the words `:`, `;`, `literal`, and their direct
        // dependencies. This code *is* Forth, just not written as text.
        //
        // `N(name)` compiles a call to a previously defined word. Any reference to an XT that
        // should be a data value at runtime (`['] word`) must be a literal (`L(xt)`).
        let here = addr!(HERE);
        let latest = addr!(LATEST);

        // cells
        compile!(b"cells", 0, [L(SIZE), N(b"um*"), N(b"drop")]);

        // : +! ( u addr -- ) dup >r @ + r> ! ;
        compile!(b"+!", 0, [N(b"dup"), N(b">r"), N(b"@"), N(b"+"), N(b"r>"), N(b"!")]);

        // : allot ( n -- ) (here) +! ;
        compile!(b"allot", 0, [here, N(b"+!")]);

        // : , ( x -- ) here ! 1 cells allot ;
        //
        // `here` is always cell-aligned at this stage so it is safe call `,` without `align`.
        compile!(
            b",",
            0,
            [here, N(b"@"), N(b"!"), L(1), N(b"cells"), N(b"allot")]
        );

        // : literal ( x -- ) ['] (lit) , , ; immediate
        //
        // The first lit is the (lit) opcode. The second lit is the XT of (lit).
        // At runtime this executes as `(lit) xt`, compiling the XT of (lit) and then the
        // original top of stack.
        compile!(
            b"literal",
            Flags::IMMEDIATE,
            [L(Op::Lit as usize), N(b","), N(b",")]
        );

        // : ['] bl parse (find) drop literal ; immediate
        //
        // This definition does not check the flag value because bootstrap input is trusted.
        // `literal` is immediate, so in Forth, it would require `postpone literal` to compile into
        // this definition. Here we can compile it directly.
        compile!(
            b"[']",
            Flags::IMMEDIATE | Flags::BOOTSTRAP,
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
            Flags::BOOTSTRAP,
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
            Flags::IMMEDIATE,
            [
                L(Op::Exit as usize), N(b","),
                // Calculate and set bodylen.
                latest, N(b"@"),
                N(b"dup"), L((3 * SIZE).wrapping_neg()), N(b"+"),
                N(b"swap"),
                here, N(b"@"), N(b"swap"),
                N(b"dup"), N(b"(nand)"), L(1), N(b"+"), N(b"+"), // inline -
                N(b"swap"), N(b"!"),
                // Unset hidden flag.
                latest, N(b"@"),
                L((2 * SIZE).wrapping_neg()), N(b"+"), L(1), N(b"+"), N(b"dup"), N(b"c@"),
                L(Flags::HIDDEN.into()), N(b"dup"), N(b"(nand)"), N(b"(nand)"), N(b"dup"), N(b"(nand)"), // inline invert, and
                N(b"swap"), N(b"c!"),
                L(FALSE), N(b"state"), N(b"!"),
            ]
        );

        // (interpret)
        //
        // This one is thorny. First, the Forth:
        //
        //     : (interpret)
        //       begin
        //         >in @ (source-len) @ <       ( flag )
        //       while
        //         bl parse                     ( c-addr u )
        //         dup if                       ( c-addr u )
        //           2dup (find)                ( c-addr u 0 | c-addr u xt flag )
        //           ?dup if                    ( c-addr u xt flag )
        //             2swap 2drop              ( xt flag )
        //             0< (state) @ and if      ( xt )
        //               compile,               ( )
        //             else
        //               execute                ( )
        //             then
        //           else                       ( c-addr u )
        //             (number?) if             ( n )
        //               (state) @ if           ( n )
        //                 postpone literal     ( )
        //               then
        //             else                     ( c-addr u )
        //               \ TODO
        //               2drop                  ( )
        //               0 1 0 um/mod
        //             then
        //           then
        //         else                         ( c-addr u )
        //           2drop                      ( )
        //         then
        //       repeat
        //     ;
        //
        // The control flow words execute immediately and compile jumps to the current value of
        // `here`. We need to compile these jumps manually by setting labels and patching the jumps
        // in a second pass (see `compile`).
        //
        // As above, note that `postpone literal` has the effect of compiling a call to `literal`
        // into the current definition. We can compile `literal` directly here.

        // : over >r dup r> swap ;
        // : 2dup over over ;
        compile!(
            b"2dup",
            Flags::BOOTSTRAP,
            [
                N(b">r"), N(b"dup"), N(b"r>"), N(b"swap"), // over
                N(b">r"), N(b"dup"), N(b"r>"), N(b"swap"), // over
            ]
        );

        // : 2drop drop drop ;
        compile!(
            b"2drop",
            Flags::BOOTSTRAP,
            [N(b"drop"), N(b"drop")]
        );

        // : rot >r swap r> swap ;
        // : 2swap rot >r rot r> ;
        compile!(
            b"2swap",
            Flags::BOOTSTRAP,
            [
                N(b">r"), N(b"swap"), N(b"r>"), N(b"swap"),
                N(b">r"),
                N(b">r"), N(b"swap"), N(b"r>"), N(b"swap"),
                N(b"r>"),
            ]
        );

        // : ?dup dup if dup then ;
        compile!(
            b"?dup",
            Flags::BOOTSTRAP,
            [N(b"dup"), If, N(b"dup"), Then]
        );

        compile!(
            b"and",
            Flags::BOOTSTRAP,
            [N(b"(nand)"), N(b"dup"), N(b"(nand)")]
        );

        compile!(
            b"<",
            // TODO: Figure out how to hide this before parse-name.
            Flags::EMPTY,
            [N(b"dup"), N(b"(nand)"), L(1), N(b"+"), N(b"+"), N(b"0<")]
        );

        let state = addr!(STATE);
        let to_in = addr!(TO_IN);
        let source_len = addr!(SOURCE_LEN);
        compile!(
            b"(interpret)",
            Flags::BOOTSTRAP,
            [
                Begin,
                    to_in, N(b"@"), source_len, N(b"@"), N(b"<"),
                While,
                    L(BL), N(b"parse"),
                    N(b"dup"), If,
                        N(b"2dup"), N(b"(find)"),
                        N(b"?dup"), If,
                            N(b"2swap"), N(b"2drop"),
                            N(b"0<"), state, N(b"@"), N(b"and"), If,
                                N(b"compile,"),
                            Else,
                                N(b"execute"),
                            Then,
                        Else,
                            N(b"(number?)"), If,
                                state, N(b"@"), If,
                                    // NOTE: `postpone literal` compiles to `literal`.
                                    N(b"literal"),
                                Then,
                            Else,
                                N(b"2drop"),
                                L(0), L(1), L(0), N(b"um/mod"),
                            Then,
                        Then,
                    Else,
                        N(b"2drop"),
                    Then,
                Repeat,
            ]
        );
        Ok(())
    }

    fn load_kernel(&mut self) -> Result<()> {
        for line in KERNEL.split(|&b| b == b'\n') {
            if !line.is_empty() {
                self.dict().set_source(line)?;
                let xt = self
                    .dict()
                    .find(b"(interpret)")?
                    .map(|(xt, _)| xt)
                    .ok_or(KernelError::MissingEntryPoint("(interpret)"))?;
                self.execute(xt)?;
            }
        }
        Ok(())
    }

    fn compile_environment(&mut self) -> Result<()> {
        let flag = |b: bool| -> usize { if b { TRUE } else { FALSE } };
        self.compile(
            b"(/counted-string)",
            Flags::EMPTY,
            &[Token::Lit(self.env.counted_string)],
        )?;
        self.compile(
            b"(/hold)",
            Flags::EMPTY,
            &[Token::Lit(self.env.config.hold)],
        )?;
        self.compile(b"(/pad)", Flags::EMPTY, &[Token::Lit(self.env.config.pad)])?;
        self.compile(
            b"(address-unit-bits)",
            Flags::EMPTY,
            &[Token::Lit(self.env.address_unit_bits)],
        )?;
        self.compile(
            b"(floored)",
            Flags::EMPTY,
            &[Token::Lit(flag(self.env.floored))],
        )?;
        self.compile(
            b"(max-char)",
            Flags::EMPTY,
            &[Token::Lit(self.env.max_char)],
        )?;
        let (lo, hi): (usize, usize) = Double(self.env.max_d.0 as _).into();
        self.compile(b"(max-d)", Flags::EMPTY, &[Token::Lit(lo), Token::Lit(hi)])?;
        self.compile(
            b"(max-n)",
            Flags::EMPTY,
            &[Token::Lit(self.env.max_n as usize)],
        )?;
        self.compile(b"(max-u)", Flags::EMPTY, &[Token::Lit(self.env.max_u)])?;
        let (lo, hi): (usize, usize) = self.env.max_ud.into();
        self.compile(b"(max-ud)", Flags::EMPTY, &[Token::Lit(lo), Token::Lit(hi)])?;
        self.compile(
            b"(return-stack-cells)",
            Flags::EMPTY,
            &[Token::Lit(self.env.config.return_stack_cells)],
        )?;
        self.compile(
            b"(stack-cells)",
            Flags::EMPTY,
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
            self.compile(name, Flags::EMPTY, &[Token::Lit(self.layout_base + offset)])?;
        }
        Ok(())
    }

    fn compile_wrappers(&mut self) -> Result<()> {
        use Token::Name as N;
        self.compile(
            b"parse",
            Flags::EMPTY,
            &[
                N(b"(source-addr)"),
                N(b"@"),
                N(b"(source-len)"),
                N(b"@"),
                N(b">in"),
                N(b"@"),
                N(b"(parse)"),
                N(b">in"),
                N(b"!"),
            ],
        )?;
        self.compile(
            b"compile,",
            Flags::EMPTY,
            &[
                N(b"(here)"),
                N(b"@"),
                N(b"(compile,)"),
                N(b"(here)"),
                N(b"!"),
            ],
        )?;
        self.compile(
            b">number",
            Flags::EMPTY,
            &[N(b"base"), N(b"@"), N(b"(>number)")],
        )?;
        self.compile(
            b"(number?)",
            Flags::EMPTY,
            &[N(b"base"), N(b"@"), N(b"(number)")],
        )?;
        Ok(())
    }

    fn compile(&mut self, name: &[u8], flags: Flags, body: &[Token]) -> Result<usize> {
        let xt = self.dict().create(name, (flags | Flags::COLON).into())?;
        self.dict().set_latest(xt)?;
        self.dict().set_here(xt)?;

        let mut labels = [0usize; 16];
        let mut depth = 0;
        for &token in body {
            match token {
                Token::Lit(x) => {
                    self.dict().comma(Op::Lit as usize)?;
                    self.dict().comma(x)?;
                }
                Token::Name(name) => {
                    let xt = self
                        .dict()
                        .find(name)?
                        .map(|(xt, _)| xt)
                        .ok_or(Error::Throw(Ior::UNDEFINED_WORD))?;
                    self.push(xt)?;
                    self.compile_comma()?;
                }
                Token::If | Token::While => {
                    let xt = self.dict().find(b"(jmpz)")?.unwrap().0; // TODO: unwrap
                    self.push(xt)?;
                    self.compile_comma()?;
                    let hole = self.dict().here()?;
                    self.dict().comma(0)?;
                    labels[depth] = hole;
                    depth += 1;
                }
                Token::Else => {
                    depth -= 1;
                    let orig = labels[depth];
                    let xt = self.dict().find(b"(jmp)")?.unwrap().0; // TODO: unwrap
                    self.push(xt)?;
                    self.compile_comma()?;
                    let hole = self.dict().here()?;
                    self.dict().comma(0)?;
                    let here = self.dict().here()?;
                    self.data.write_cell(orig, here)?;
                    labels[depth] = hole;
                    depth += 1;
                }
                Token::Then => {
                    depth -= 1;
                    let hole = labels[depth];
                    let here = self.dict().here()?;
                    self.data.write_cell(hole, here)?;
                }
                Token::Begin => {
                    let here = self.dict().here()?;
                    labels[depth] = here;
                    depth += 1;
                }
                Token::Repeat => {
                    depth -= 1;
                    let hole = labels[depth];
                    depth -= 1;
                    let dest = labels[depth];
                    let xt = self.dict().find(b"(jmp)")?.unwrap().0; // TODO: unwrap
                    self.push(xt)?;
                    self.compile_comma()?;
                    self.dict().comma(dest)?;
                    let here = self.dict().here()?;
                    self.data.write_cell(hole, here)?;
                }
            }
        }
        self.dict().comma(Op::Exit as usize)?;
        let here = self.dict().here()?;
        self.data
            .write_cell(Header::new(xt).bodylen_addr(), here - xt)?;
        Ok(xt)
    }

    fn compile_comma(&mut self) -> Result<()> {
        let xt = self
            .dict()
            .find(b"(compile,)")?
            .map(|(xt, _)| xt)
            .ok_or(Error::Throw(Ior::UNDEFINED_WORD))?;
        let here = self.dict().here()?;
        self.push(here)?;
        self.vm.enter(&mut self.data, xt)?;
        let here = self.pop()?;
        self.dict().set_here(here)
    }

    fn register_builtin(&mut self, name: &[u8], f: Builtin<M, I>, flags: Flags) -> Result<()> {
        let idx = self.builtins_len;
        if idx >= MAX_BUILTINS {
            return Err(KernelError::BuiltinTableFull.into());
        }
        self.builtins[idx] = Some(f);
        self.builtins_len += 1;
        let cfa = self.define(name, flags)?;
        let instr = PackedInstr::new(Op::Yield, cfa, idx)?;
        Ok(self.data.write_cell(cfa, instr.into())?)
    }

    fn define(&mut self, name: &[u8], flags: Flags) -> Result<usize> {
        let cfa = self
            .dict()
            .create(name, (flags | Flags::PRIMITIVE).into())?;
        self.dict().set_here(cfa + SIZE)?;
        self.dict().set_latest(cfa)?;
        Ok(cfa)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{Flags, Header, Info};
    use crate::io::NoIo;

    #[test]
    fn tag_boot_words_with_kind() {
        let mut k = Kernel::new([0u8; 65536], NoIo, Config::default())
            .boot()
            .unwrap();
        let mut flags = |name: &[u8]| {
            let (xt, _) = k.dict().find(name).unwrap().unwrap();
            let header = Header::new(xt);
            let info: Info = k.data.read_cell(header.info_addr()).unwrap().into();
            info.flags()
        };
        assert!(flags(b"dup").contains(Flags::PRIMITIVE));
        assert!(flags(b"(find)").contains(Flags::PRIMITIVE));
        assert!(flags(b"cells").contains(Flags::COLON));
    }
}
