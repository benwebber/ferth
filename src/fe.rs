//! The outer interpreter.
use core::mem::offset_of;

use crate::counted::CountedStr31;
use crate::types::Double;
use crate::{Error, Result};

use super::data::{Data, Mem};
use super::io::{Io, NoIo};
use super::parser;
use super::types::Cell;
use super::vm::{Op, Stop, Vm};

pub const IMMEDIATE: u8 = 0b01;
pub const HIDDEN: u8 = 0b10;

/// The maximum word length in bytes.
const MAX_WORD_LEN: usize = 31;
/// The maximum number of builtins in the builtins table.
const MAX_BUILTINS: usize = 256;
/// The size of the terminal input buffer.
const INPUT_BUFFER_SIZE: usize = 256;

pub const BL: Cell = Cell(0x20);

const CORE: &[u8] = include_bytes!("core.fth");
const CORE_EXT: &[u8] = include_bytes!("core-ext.fth");

pub type Builtin<M, I> = fn(&mut Fe<M, I>) -> Result<()>;

const INFO_FROM_CFA: usize = 2 * Vm::SIZE;

#[derive(Clone, Copy)]
enum Token {
    Lit(usize),
    Xt(usize),
}

pub struct Environment {
    pub counted_string: usize,
    pub hold: usize,
    pub pad: usize,
    pub address_unit_bits: usize,
    pub floored: bool,
    pub max_char: usize,
    pub max_d: i128,
    pub max_n: isize,
    pub max_u: usize,
    pub max_ud: Double,
    pub return_stack_cells: usize,
    pub stack_cells: usize,
}

/// The layout of the data space.
///
/// Represents the first region of memory after the VM's internal regions, containing system
/// variables such as `here`.
#[repr(C)]
pub struct Layout {
    /// The data space pointer (`(here)`).
    here: usize,
    /// The XT of the latest word defined (`(latest)`).
    latest: usize,
    /// The system compilation state (`state`).
    state: usize,
    /// The current numeral system base (`base`).
    base: usize,
    /// The current offset in the input buffer (`>in`).
    to_in: usize,
    /// The current input buffer address (`(source-addr)`).
    source_addr: usize,
    /// The current input buffer length (`(source-len)`).
    source_len: usize,
    /// The initial data stack pointer (`(sp0)`).
    sp0: usize,
    /// The initial return stack pointer (`(rp0)`).
    rp0: usize,
    /// The terminal input buffer.
    input: [u8; INPUT_BUFFER_SIZE],
}

impl Layout {
    /// The offset of the data space pointer (`(here)`)
    pub const HERE: usize = offset_of!(Self, here);
    pub const LATEST: usize = offset_of!(Self, latest);
    pub const STATE: usize = offset_of!(Self, state);
    pub const BASE: usize = offset_of!(Self, base);
    pub const TO_IN: usize = offset_of!(Self, to_in);
    pub const SOURCE_ADDR: usize = offset_of!(Self, source_addr);
    pub const SOURCE_LEN: usize = offset_of!(Self, source_len);
    pub const SP0: usize = offset_of!(Self, sp0);
    pub const RP0: usize = offset_of!(Self, rp0);
    pub const INPUT: usize = offset_of!(Self, input);
    pub const DATA: usize = size_of::<Self>();
}

/// The outer interpreter.
pub struct Fe<M: Mem = [u8; 65536], I: Io = NoIo> {
    vm: Vm,
    data: Data<M>,
    io: I,
    // lookup table for Op CFAs
    op_xts: [usize; 256],
    // The XT of `,`. Saved during bootstrap for `postpone`.
    // TODO: Figure out a better way to define both comma and postpone.
    comma_xt: usize,
    builtins: [Option<Builtin<M, I>>; MAX_BUILTINS],
    builtins_len: usize,
    layout_base: usize,
}

impl<M: Mem, I: Io> Fe<M, I> {
    pub fn new(mem: M, io: I) -> Result<Self> {
        let ds_len = 64;
        let rs_len = 64;
        if !Vm::layout_ok(ds_len, rs_len) {
            return Err(Error::StacksTooSmall);
        }
        let data = Data::new(mem);
        let vm = Vm::new(ds_len, rs_len);
        let layout_base = vm.reserved();
        let mut fe = Self {
            vm,
            data,
            io,
            builtins: [None; MAX_BUILTINS],
            builtins_len: 0,
            op_xts: [0; 256],
            comma_xt: 0,
            layout_base,
        };
        fe.bootstrap(ds_len, rs_len)?;
        Ok(fe)
    }

    /// Push an item to the data stack.
    ///
    /// ```text
    /// ( -- x )
    /// ```
    pub fn push<T: Into<Cell>>(&mut self, x: T) -> Result<()> {
        let x: Cell = x.into();
        self.vm.push(&mut self.data, x.into())?;
        Ok(())
    }

    /// Pop an item from the data stack.
    ///
    /// ```text
    /// ( x --  )
    /// ```
    pub fn pop(&mut self) -> Result<Cell> {
        Ok(self.vm.pop(&mut self.data).map(Cell)?)
    }

    /// Evaluate Forth code.
    pub fn evaluate(&mut self, code: &[u8]) -> Result<()> {
        if code.len() > INPUT_BUFFER_SIZE {
            return Err(Error::LineTooLong);
        }
        let input_addr = self.layout_addr(Layout::INPUT);
        self.data.write(input_addr, code)?;
        self.data
            .write_cell(self.layout_addr(Layout::SOURCE_ADDR), input_addr)?;
        self.data
            .write_cell(self.layout_addr(Layout::SOURCE_LEN), code.len())?;
        self.data.write_cell(self.layout_addr(Layout::TO_IN), 0)?;
        let result = self.interpret();
        if result.is_err() {
            let _ = self.data.write_cell(self.layout_addr(Layout::STATE), 0);
        }
        result
    }

    /// Reset the data and return stacks.
    pub fn reset(&mut self) {
        self.vm.reset()
    }

    pub fn stack(&self) -> impl Iterator<Item = usize> + '_ {
        self.vm.stack(&self.data)
    }

    /// Bootstrap Forth primitives.
    ///
    /// This function hand compiles a minimal set of primitive Forth words used to bootstrap a
    /// working Forth system. Notably, it even bootstraps the compiler words like `:` and `;`.
    #[rustfmt::skip]
    fn bootstrap(&mut self, ds_len: usize, rs_len: usize) -> Result<()> {
        macro_rules! compile {
            ($s:expr, $flags:expr, $code:expr) => {
                self.compile($s, $flags, $code, &[])?;
            };
            ($s:expr, $flags:expr, $code:expr, [$($body:expr),* $(,)?]) => {
                self.compile($s, $flags, $code, &[$($body),*])?;
            };
            ($name:ident, $s:expr, $flags:expr, $code:expr) => {
                let $name = Xt(self.compile($s, $flags, $code, &[])?);
            };
            ($name:ident, $s:expr, $flags:expr, $code:expr, [$($body:expr),* $(,)?]) => {
                let $name = Xt(self.compile($s, $flags, $code, &[$($body),*])?);
            };
        }

        macro_rules! addr {
            ($name:ident) => {
                L(self.layout_addr(Layout::$name))
            };
        }

        use Token::{Lit as L, Xt};

        // 1. Reserve cells for system variables.
        let variables: &[(&[u8], usize, usize)] = &[
            (b"(here)", Layout::HERE, self.layout_base + Layout::DATA),
            (b"(latest)", Layout::LATEST, 0),
            (b"(source-addr)", Layout::SOURCE_ADDR, self.layout_base + Layout::INPUT),
            (b"(source-len)", Layout::SOURCE_LEN, 0),
            (b">in", Layout::TO_IN, 0),
            (b"base", Layout::BASE, 10),
            (b"state", Layout::STATE, 0),
            (b"(sp0)", Layout::SP0, Vm::DS_ADDR),
            (b"(rp0)", Layout::RP0, self.vm.rs_addr() + Vm::SIZE),
        ];
        for (_, offset, value) in variables {
            self.data.write_cell(self.layout_base + offset, *value)?;
        }

        // 2. Compile inner interpreter words ("opcodes").
        //
        // The inner interpreter implements these words directly. They comprise the most
        // fundamental set of execution, stack, memory, and arithmetic operations.
        let opcodes: &[(&[u8], Op)] = &[
            (b"!", Op::Store),
            (b"(docol)", Op::DoCol),
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
            (b"r>", Op::RFrom),
            (b"swap", Op::Swap),
            (b"lshift", Op::LShift),
            (b"rshift", Op::RShift),
            (b"um/mod", Op::UmDivMod),
            // r@ cannot be a : definition because Op::DoCol puts its return address on top of the
            // return stack before r@ executes. r@ returns that instead of whatever was previously
            // on the top of the return stack. The opcode executes directly.
            (b"r@", Op::RFetch),
        ];
        for (name, op) in opcodes {
            let xt = self.compile(name, 0, *op, &[])?;
            self.op_xts[*op as usize] = xt;
        }

        // 3. Compile outer interpreter words ("builtins").
        //
        // These words concern parsing and I/O. They may exist as builtins for several reasons.
        // The parsing words are difficult, or inefficient, to express in Forth. The inner
        // interpreter lacks any I/O facilities, so the outer interpret naturally has to provide
        // these.
        let mut header = 0;
        let mut parse = 0;
        let builtins: &[(&[u8], Builtin<M, I>, u8)] = &[
            (b"'", Self::tick, 0),
            (b"(interpret)", Self::interpret, 0),
            (b"(number)", Self::number, 0),
            (b"emit", Self::emit, 0),
            (b"execute", Self::execute, 0),
            (b"(find)", Self::find, 0),
            (b"key", Self::key, 0),
            (b"parse", Self::parse, 0),
            (b"postpone", Self::postpone, IMMEDIATE),
            (b"refill", Self::refill, 0),
            (b"(header)", Self::header, 0),
            (b">number", Self::to_number, 0),
        ];
        for (name, f, flags) in builtins {
            self.register_builtin(name, *f, *flags)?;
            // HACK: Figure out a better way to capture these XTs.
            if name == b"(header)" {
                header = self.data.read_cell(self.layout_addr(Layout::LATEST))?;
            } else if name == b"parse" {
                parse = self.data.read_cell(self.layout_addr(Layout::LATEST))?;
            }
        }

        // 4. Compile compiler words.
        //
        // This sequence hand-compiles the words `:`, `;`, `create`, and their direct dependencies.
        // This code *is* Forth, just not written as text. Consider it Forth "assembly".
        //
        // Any reference to an XT that should be a data value at runtime must be a literal
        // (`L(xt)`). Do not use `'`, which consumes from the input stream.
        // TODO: Remove this warning after implementing errors and removing `'` from the builtins.
        let header = Xt(header);
        let parse = Xt(parse);

        let nand = Xt(self.op_xt(Op::Nand));
        let add = Xt(self.op_xt(Op::Add));
        let fetch = Xt(self.op_xt(Op::Fetch));
        let store = Xt(self.op_xt(Op::Store));
        let to_r = Xt(self.op_xt(Op::ToR));
        let r_from = Xt(self.op_xt(Op::RFrom));
        let ummul = Xt(self.op_xt(Op::UmMul));
        let c_fetch = Xt(self.op_xt(Op::CFetch));
        let c_store = Xt(self.op_xt(Op::CStore));
        let swap = Xt(self.op_xt(Op::Swap));
        let sp_fetch = Xt(self.op_xt(Op::SpFetch));
        let drop = Xt(self.op_xt(Op::Drop));
        let bl = L(usize::from(BL));

        // : dup ( x -- x x ) (sp@) @ [-SIZE] + ;
        //
        // We have to use -SIZE because `-` is not available yet.
        compile!(dup, b"dup", 0, Op::DoCol, [sp_fetch, L(Vm::SIZE.wrapping_neg()), add, fetch]);
        // : invert ( x1 -- x2 ) dup (nand) ;
        compile!(invert, b"invert", 0, Op::DoCol, [dup, nand]);
        // : or ( x1 x2 -- x3 ) invert swap invert (nand) ;
        compile!(or, b"or", 0, Op::DoCol, [invert, swap, invert, nand]);
        // : and ( x1 x2 -- x3 ) (nand) invert ;
        compile!(and, b"and", 0, Op::DoCol, [nand, invert]);
        // : - ( n1 n2 -- n3 ) invert 1+ + ;
        compile!(minus, b"-", 0, Op::DoCol, [invert, L(1), add, add]);
        // cells
        compile!(cells, b"cells", 0, Op::DoCol, [L(Vm::SIZE), ummul, drop]);
        // : +! ( u addr -- ) dup >r @ + r> ! ;
        compile!(plus_store, b"+!", 0, Op::DoCol, [dup, to_r, fetch, add, r_from, store]);
        // : allot ( n -- ) (here) +! ;
        compile!(allot, b"allot", 0, Op::DoCol, [addr!(HERE), plus_store]);
        // : aligned ( addr -- a-addr ) 1 cells 1- + 1 cells 1- invert and ;
        compile!(
            aligned,
            b"aligned",
            0,
            Op::DoCol,
            [L(1), cells, L(-1isize as usize), add, add, L(1), cells, L(-1isize as usize), add, invert, and]
        );
        // : align ( -- ) here aligned here - allot ;
        compile!(
            align,
            b"align",
            0,
            Op::DoCol,
            [addr!(HERE), fetch, aligned, addr!(HERE), fetch, minus, allot]
        );
        // : , ( x -- ) align here ! 1 cells allot ;
        compile!(
            comma,
            b",",
            0,
            Op::DoCol,
            [align, addr!(HERE), fetch, store, L(1), cells, allot]
        );
        let Xt(comma_xt) = comma else { unreachable!() };
        self.comma_xt = comma_xt;

        // : literal ( x -- ) ['] (lit) , , ; immediate
        //
        // The first lit is the (lit) opcode. The second lit is the XT of (lit).
        // At runtime this executes as `(lit) xt`, compiling the XT of (lit) and then the
        // original top of stack.
        compile!(
            b"literal",
            IMMEDIATE,
            Op::DoCol,
            [L(self.op_xt(Op::Lit)), comma, comma]
        );

        // Finally, compile the compilation words.

        // : ] true state ! ;
        compile!(rbracket, b"]", 0, Op::DoCol, [L(usize::MAX), addr!(STATE), store]);
        // : [ false state ! ;
        compile!(lbracket, b"[", IMMEDIATE, Op::DoCol, [L(0), addr!(STATE), store]);

        // : create ( "<spaces>name" -- ) bl parse (header) (docreate) , 0 , ;
        compile!(
            b"create",
            0,
            Op::DoCol,
            [bl, parse, header, L(Op::DoCreate as usize), comma, L(0), comma]
        );

        // (hidden-flag)
        compile!(hidden_flag, b"(hidden-flag)", 0, Op::DoCol, [L(HIDDEN.into())]);
        // (immediate-flag)
        compile!(b"(immediate-flag)", 0, Op::DoCol, [L(IMMEDIATE.into())]);

        // (flags-addr)
        //
        // Return the address of the flags byte, calculated from the code address (XT).
        compile!(
            flags_addr,
            b"(flags-addr)",
            0,
            Op::DoCol,
            [L((2 * Vm::SIZE).wrapping_neg()), add, L(1), add]
        );

        // : :
        //   bl parse (header)
        //   (latest) @ (flags-addr) dup c@ (hidden-flag) or swap c!
        //   ' (docol) @ ,
        //   ]
        // ;
        //
        // Parse a word, create a definition for it, mark it hidden, and compile `DoCol` to the
        // code address.
        compile!(
            b":",
            0,
            Op::DoCol,
            [
                bl, parse, header,
                addr!(LATEST), fetch, flags_addr, dup, c_fetch, hidden_flag, or, swap, c_store,
                L(self.op_xt(Op::DoCol)), fetch, comma,
                rbracket,
            ]
        );

        // : ;
        //   ['] (exit) ,
        //   (latest) @ (flags-addr) dup c@ (hidden-flag) invert and swap c!
        //   [
        // ; immediate
        //
        // Compile a literal to compile `exit`, then unset the hidden flag.
        compile!(
            b";",
            IMMEDIATE,
            Op::DoCol,
            [
                L(self.op_xt(Op::Exit)), comma,
                addr!(LATEST), fetch, flags_addr, dup, c_fetch, hidden_flag, invert, and, swap, c_store,
                lbracket,
            ]
        );

        // 5. Initialize system variables.
        for (name, offset, _) in variables {
            self.compile(name, 0, Op::DoCol, &[Token::Lit(self.layout_base + offset)])?;
        }

        // 6. Initialize environment constants.
        self.compile_environment(ds_len, rs_len)?;

        // 7. Bootstrap core wordlists.
        //
        // With the compiler words bootstrapped, we can bootstrap the rest of the system in Forth.
        for src in &[CORE, CORE_EXT] {
            for line in src.split(|&b| b == b'\n') {
                if !line.is_empty() {
                    self.evaluate(line)?;
                }
            }
        }

        Ok(())
    }

    fn compile_environment(&mut self, ds_len: usize, rs_len: usize) -> Result<()> {
        let env = Environment {
            counted_string: 255,
            hold: 64,
            pad: 84,
            address_unit_bits: 8,
            floored: false,
            max_char: 255,
            max_d: i128::MAX,
            max_n: isize::MAX,
            max_u: usize::MAX,
            max_ud: Double::MAX,
            return_stack_cells: rs_len,
            stack_cells: ds_len,
        };
        let flag = |b: bool| -> usize { if b { usize::MAX } else { 0 } };
        self.compile(
            b"(/counted-string)",
            0,
            Op::DoCol,
            &[Token::Lit(env.counted_string)],
        )?;
        self.compile(b"(/hold)", 0, Op::DoCol, &[Token::Lit(env.hold)])?;
        self.compile(b"(/pad)", 0, Op::DoCol, &[Token::Lit(env.pad)])?;
        self.compile(
            b"(address-unit-bits)",
            0,
            Op::DoCol,
            &[Token::Lit(env.address_unit_bits)],
        )?;
        self.compile(b"(floored)", 0, Op::DoCol, &[Token::Lit(flag(env.floored))])?;
        self.compile(b"(max-char)", 0, Op::DoCol, &[Token::Lit(env.max_char)])?;
        let (lo, hi): (usize, usize) = Double(env.max_d as _).into();
        self.compile(b"(max-d)", 0, Op::DoCol, &[Token::Lit(lo), Token::Lit(hi)])?;
        self.compile(b"(max-n)", 0, Op::DoCol, &[Token::Lit(env.max_n as usize)])?;
        self.compile(b"(max-u)", 0, Op::DoCol, &[Token::Lit(env.max_u)])?;
        let (lo, hi): (usize, usize) = env.max_ud.into();
        self.compile(b"(max-ud)", 0, Op::DoCol, &[Token::Lit(lo), Token::Lit(hi)])?;
        self.compile(
            b"(return-stack-cells)",
            0,
            Op::DoCol,
            &[Token::Lit(env.return_stack_cells)],
        )?;
        self.compile(
            b"(stack-cells)",
            0,
            Op::DoCol,
            &[Token::Lit(env.stack_cells)],
        )?;
        Ok(())
    }

    /// Create a new dictionary header.
    ///
    /// ```text
    /// (header) ( c-addr u -- )
    /// ```
    ///
    /// The header starts with a variable-length `pad` field that ensures the `info` field always
    /// aligns to a cell address.
    ///
    /// The length of the name follows as a single byte, then the bytes of the name.
    ///
    /// The `info` field packs the flags into the least significant byte and the length into the
    /// next byte. It currently reserves two additional bytes of space.
    ///
    /// The `link` field links to the `code` field of the next word in the dictionary.
    ///
    /// The `code` field contains an [`Op`] code. The compiled `data `of the word, if it exists,
    /// follows the `code` field.
    ///
    /// Assuming a 32-bit cell size, the header looks like this in memory:
    ///
    /// ```text
    ///  0 1 2 3 4 5 6 7 8 9 a b c d e f 0 1 2 3 4 5 6 7 8 9 a b c d e f
    /// +---------------+---------------+-------------------------------+
    /// |      pad...   |      len      |             name...           |
    /// +---------------+---------------+-------------------------------+
    /// |                              name...                          |
    /// +-------------------------------+---------------+---------------+
    /// |            info (reserved)    |   info (len)  | info (flags)  |
    /// +-------------------------------+---------------+---------------+
    /// |                              link                             |
    /// +---------------------------------------------------------------+
    /// |                              code                             |
    /// +---------------------------------------------------------------+
    /// |                              data...                          |
    /// +---------------------------------------------------------------+
    /// ```
    ///
    /// After `(header)` executes, `here` points to the `code` field address.
    fn header(&mut self) -> Result<()> {
        let len = usize::from(self.pop()?);
        let addr = usize::from(self.pop()?);
        let mut buf = [0u8; MAX_WORD_LEN];
        buf[..len].copy_from_slice(self.data.read(addr, len)?);
        let cfa = self.write_header(&buf[..len], 0)?;
        self.data
            .write_cell(self.layout_addr(Layout::LATEST), cfa)?;
        self.data.write_cell(self.layout_addr(Layout::HERE), cfa)?;
        Ok(())
    }

    /// Parse digits and add them to an accumulator.
    ///
    /// ```text
    /// >number ( ud1 c-addr1 u1 -- ud2 c-addr2 u2 )
    /// ```
    #[allow(clippy::wrong_self_convention)]
    fn to_number(&mut self) -> Result<()> {
        let u: usize = self.pop()?.into();
        let caddr: usize = self.pop()?.into();
        let hi = self.pop()?;
        let lo = self.pop()?;
        let acc = Double::from((lo, hi));
        let bytes = self.data.read(caddr, u)?;
        // TODO: Check base size.
        let base = self.data.read_cell(self.layout_addr(Layout::BASE))? as u32;
        let (acc, rest) = parser::to_number(acc, bytes, base);
        let len = bytes.len() - rest.len();
        let (lo, hi): (Cell, Cell) = acc.into();
        let caddr2 = caddr + len;
        let u2 = rest.len();
        self.push(lo)?;
        self.push(hi)?;
        self.push(Cell(caddr2))?;
        self.push(Cell(u2))
    }

    fn write_header(&mut self, name: &[u8], flags: u8) -> Result<usize> {
        let len: u8 = name
            .len()
            .try_into()
            .map_err(|_| Error::CountedStrTooLong(name.len()))?;
        let latest = self.data.read_cell(self.layout_addr(Layout::LATEST))?;
        let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
        // pad the name so as to always align info
        let pad = (Vm::SIZE - ((here + 1 + len as usize) % Vm::SIZE)) % Vm::SIZE;
        // name
        let nfa = here + pad;
        self.data.write_char(nfa, len)?;
        self.data.write(nfa + 1, name)?;
        // info
        let info = nfa + 1 + len as usize;
        self.data.write_cell(info, pack_info(flags, len))?;
        self.data.write_cell(info + Vm::SIZE, latest)?;
        // code
        let cfa = info + 2 * Vm::SIZE;
        Ok(cfa)
    }

    fn compile(&mut self, name: &[u8], flags: u8, code: Op, body: &[Token]) -> Result<usize> {
        let xt = self.define(name, code, flags)?;
        let lit_xt = self.op_xts[Op::Lit as usize];
        for &token in body {
            match token {
                Token::Xt(x) => self.comma(x)?,
                Token::Lit(v) => {
                    self.comma(lit_xt)?;
                    self.comma(v)?;
                }
            }
        }
        if code == Op::DoCol {
            self.comma(self.op_xts[Op::Exit as usize])?;
        }
        Ok(xt)
    }

    fn comma(&mut self, val: usize) -> Result<()> {
        let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
        self.data.write_cell(here, val)?;
        self.data
            .write_cell(self.layout_addr(Layout::HERE), here + Vm::SIZE)?;
        Ok(())
    }

    /// ( "<spaces>" -- c-addr )
    // TODO: Remove after implementing ', (interpret), find, etc. in Forth.
    fn parse_word(&mut self, delim: u8) -> Result<usize> {
        let src = self.data.read_cell(self.layout_addr(Layout::SOURCE_ADDR))?;
        let src_len = self.data.read_cell(self.layout_addr(Layout::SOURCE_LEN))?;
        let mut to_in = self.data.read_cell(self.layout_addr(Layout::TO_IN))?;
        let is_delim = |c: u8| {
            if delim == b' ' {
                c.is_ascii_whitespace()
            } else {
                c == delim
            }
        };
        while to_in < src_len && is_delim(self.data.read_char(src + to_in)?) {
            to_in += 1;
        }
        self.data
            .write_cell(self.layout_addr(Layout::TO_IN), to_in)?;
        self.push(Cell(delim as usize))?;
        self.parse()?;
        let u = usize::from(self.pop()?);
        let caddr = usize::from(self.pop()?);
        // TODO: return error instead of truncating
        let len = u.min(255);
        let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
        if here + 1 + len > self.data.len() {
            return Err(crate::vm::VmError::AddressOutOfRange(here).into());
        }
        // We need to read into a temporary buffer because `read` takes an immutable reference and
        // `write` takes a mutable one. `Data` could provide a `copy_within` method to avoid this.
        let mut buf = [0u8; 256];
        buf[..len].copy_from_slice(self.data.read(caddr, len)?);
        self.data.write_char(here, len as u8)?;
        self.data.write(here + 1, &buf[..len])?;
        Ok(here)
    }

    /// A variant of `find` that reads a Forth string `( c-addr u )` instead of a counted string `(
    /// c-addr )`.
    ///
    /// ```text
    /// (find) ( c-addr u -- 0 | xt 1 | xt -1 )
    /// ```
    ///
    /// Similar to [`search-wordlist`] except it does not accept a wordlist ID.
    ///
    /// [`search-wordlist`]: https://forth-standard.org/standard/search/SEARCH-WORDLIST
    fn find(&mut self) -> Result<()> {
        let len = usize::from(self.pop()?);
        let addr = usize::from(self.pop()?);
        if len > MAX_WORD_LEN {
            return self.push(Cell::ZERO);
        }

        let mut xt = self.data.read_cell(self.layout_addr(Layout::LATEST))?;
        let mut found: Option<(usize, isize)> = None;
        while xt != 0 {
            let info = self.data.read_cell(xt - INFO_FROM_CFA)?;
            let flags = (info >> 8) as u8;
            let wlen = info & 0xFF;
            if flags & HIDDEN == 0 && wlen == len {
                let name_at = xt - INFO_FROM_CFA - wlen;
                let a = self.data.read(addr, len)?;
                let b = self.data.read(name_at, wlen)?;
                if a.eq_ignore_ascii_case(b) {
                    let flag = if flags & IMMEDIATE != 0 { 1 } else { -1 };
                    found = Some((xt, flag));
                    break;
                }
            }
            xt = self.data.read_cell(xt - Vm::SIZE)?;
        }

        match found {
            Some((xt, flag)) => {
                self.push(Cell(xt))?;
                self.push(Cell(flag as usize))
            }
            None => self.push(Cell::ZERO),
        }
    }

    /// ( "<spaces>name" -- xt )
    // TODO: after implementing errors, move this to Forth
    fn tick(&mut self) -> Result<()> {
        let caddr = self.parse_word(BL.0 as u8)?;
        let len = self.data.read_char(caddr)? as usize;
        self.push(Cell(caddr + 1))?;
        self.push(Cell(len))?;
        self.find()?;
        let flag = self.pop()?.to_isize();
        if flag == 0 {
            return Err(self.undefined(caddr));
        }
        Ok(())
    }

    /// Parse a number.
    ///
    /// ```text
    /// (number) ( c-addr -- n 1 | c-addr 0 )
    /// ```
    // TODO: Reimplement and expose this as `>number`.
    fn number(&mut self) -> Result<()> {
        let caddr = usize::from(self.pop()?);
        let len = self.data.read_char(caddr)? as usize;
        let base = self.data.read_cell(self.layout_addr(Layout::BASE))?;

        if let Some(n) = parser::parse_num(self.data.read(caddr + 1, len)?, base as u32) {
            self.push(Cell(n))?;
            self.push(Cell(1))
        } else {
            self.push(Cell(caddr))?;
            self.push(Cell::ZERO)
        }
    }

    // TODO: Move this to Forth.
    fn postpone(&mut self) -> Result<()> {
        let caddr = self.parse_word(BL.0 as u8)?;
        let len = self.data.read_char(caddr)? as usize;
        self.push(Cell(caddr + 1))?;
        self.push(Cell(len))?;
        self.find()?;
        let flag = self.pop()?.to_isize();
        if flag == 0 {
            return Err(self.undefined(caddr));
        }
        let xt = usize::from(self.pop()?);
        let is_immediate = flag == 1;
        if is_immediate {
            // Compile the XT directly so that the current word *executes* the target when it runs.
            self.comma(xt)
        } else {
            // Compile `(lit) xt ,` so that the current word *compiles* the target when it runs.
            self.comma(self.op_xts[Op::Lit as usize])?;
            self.comma(xt)?;
            self.comma(self.comma_xt)
        }
    }

    /// Execute the word referred to by *xt*.
    ///
    /// ```text
    /// execute ( i*x xt -- j*x )
    /// ```
    fn execute(&mut self) -> Result<()> {
        let xt = self.vm.pop(&mut self.data)?;
        let mut stop = self.vm.call(&mut self.data, xt)?;
        loop {
            match stop {
                Stop::Halt => return Ok(()),
                Stop::Yield(token) => {
                    let f = self
                        .builtins
                        .get(token.index)
                        .copied()
                        .flatten()
                        .ok_or(Error::InvalidBuiltin(token.index as u8))?;
                    f(self)?;
                    stop = self.vm.resume(&mut self.data, token)?;
                }
            }
        }
    }

    /// The main interpreter loop.
    ///
    /// <https://forth-standard.org/standard/usage#section.3.4>
    // TODO: Move this to Forth.
    fn interpret(&mut self) -> Result<()> {
        loop {
            let c_addr = self.parse_word(BL.0 as u8)?;
            if self.data.read_char(c_addr)? == 0 {
                return Ok(());
            }
            let len = self.data.read_char(c_addr)? as usize;
            self.push(Cell(c_addr + 1))?;
            self.push(Cell(len))?;
            self.find()?;
            let flag = self.pop()?.to_isize();
            let state = self.data.read_cell(self.layout_addr(Layout::STATE))?;
            if flag != 0 {
                if state == 0 || flag == 1 {
                    self.execute()?;
                } else {
                    let x = usize::from(self.pop()?);
                    self.comma(x)?;
                }
            } else {
                self.push(Cell(c_addr))?;
                self.number()?;
                let ok = self.pop()?.to_isize();
                let v = usize::from(self.pop()?);
                if ok == 1 {
                    if state != 0 {
                        self.comma(self.op_xts[Op::Lit as usize])?;
                        self.comma(v)?;
                    } else {
                        self.push(Cell(v))?;
                    }
                } else {
                    return Err(self.undefined(v));
                }
            }
        }
    }

    /// Parse the next token in the parse area.
    ///
    /// ```text
    /// parse ( char "ccc<char>" -- c-addr u )
    /// ```
    ///
    /// See [`PARSE`](https://forth-standard.org/standard/core/PARSE).
    fn parse(&mut self) -> Result<()> {
        let delim = usize::from(self.pop()?) as u8;
        let src = self.data.read_cell(self.layout_addr(Layout::SOURCE_ADDR))?;
        let src_len = self.data.read_cell(self.layout_addr(Layout::SOURCE_LEN))?;
        let mut to_in = self.data.read_cell(self.layout_addr(Layout::TO_IN))?;
        let start = to_in;
        let is_delim = |c: u8| {
            if delim == b' ' {
                c.is_ascii_whitespace()
            } else {
                c == delim
            }
        };
        while to_in < src_len && !is_delim(self.data.read_char(src + to_in)?) {
            to_in += 1;
        }
        let len = to_in - start;
        if to_in < src_len {
            to_in += 1;
        }
        self.data
            .write_cell(self.layout_addr(Layout::TO_IN), to_in)?;
        self.push(Cell(src + start))?;
        self.push(Cell(len))
    }

    /// Receive a single character from the input device.
    ///
    /// ```text
    /// key ( -- char )
    /// ```
    ///
    /// See [`KEY`](https://forth-standard.org/standard/core/KEY).
    fn key(&mut self) -> Result<()> {
        match self.io.key()? {
            Some(c) => self.push(Cell(c as usize)),
            None => Err(Error::Io),
        }
    }

    /// Display a single character.
    ///
    /// ```text
    /// emit ( x -- )
    /// ```
    ///
    /// See [`EMIT`](https://forth-standard.org/standard/core/EMIT).
    fn emit(&mut self) -> Result<()> {
        // TODO: What if the TOS is not a char?
        let c = usize::from(self.pop()?) as u8;
        self.io.emit(c)
    }

    /// Attempt to fill the input buffer from the input source.
    ///
    /// ```text
    /// refill ( -- flag )
    /// ```
    ///
    /// See [`REFILL`](https://forth-standard.org/standard/core/REFILL).
    fn refill(&mut self) -> Result<()> {
        let mut buf = [0u8; INPUT_BUFFER_SIZE];
        let input_addr = self.layout_addr(Layout::INPUT);
        match self.io.read_line(&mut buf) {
            Ok(Some(len)) => {
                self.data.write(input_addr, &buf[..len])?;
                self.data
                    .write_cell(self.layout_addr(Layout::SOURCE_ADDR), input_addr)?;
                self.data
                    .write_cell(self.layout_addr(Layout::SOURCE_LEN), len)?;
                self.data.write_cell(self.layout_addr(Layout::TO_IN), 0)?;
                self.push(Cell(!0))?; // true
                Ok(())
            }
            Ok(None) => {
                self.push(Cell::ZERO)?; // false
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn op_xt(&self, op: Op) -> usize {
        self.op_xts[op as usize]
    }

    fn layout_addr(&self, offset: usize) -> usize {
        self.layout_base + offset
    }

    fn register_builtin(&mut self, name: &[u8], f: Builtin<M, I>, flags: u8) -> Result<()> {
        let idx = self.builtins_len;
        if idx >= MAX_BUILTINS {
            return Err(Error::BuiltinTableFull);
        }
        self.builtins[idx] = Some(f);
        self.builtins_len += 1;
        self.define(name, Op::Yield, flags)?;
        self.comma(idx)
    }

    fn define(&mut self, name: &[u8], code: Op, flags: u8) -> Result<usize> {
        let cfa = self.write_header(name, flags)?;
        self.data.write_cell(cfa, code as usize)?;
        self.data
            .write_cell(self.layout_addr(Layout::HERE), cfa + Vm::SIZE)?;
        self.data
            .write_cell(self.layout_addr(Layout::LATEST), cfa)?;
        Ok(cfa)
    }

    fn undefined(&self, c_addr: usize) -> Error {
        let len = self.data.read_char(c_addr).unwrap_or(0) as usize;
        // Return an empty name for an invalid address instead of panicking.
        let bytes = self.data.read(c_addr + 1, len).unwrap_or(&[]);
        let name = core::str::from_utf8(bytes)
            .ok()
            .and_then(|s| CountedStr31::try_from(s).ok())
            .unwrap_or_default();
        Error::UndefinedWord(name)
    }
}

/// Pack word flags and length into one cell.
///
/// The flags occupy the least significant byte. The cell occupies the next most significant
/// byte.
fn pack_info(flags: u8, len: u8) -> usize {
    (len as usize) | ((flags as usize) << 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestFe = Fe;

    #[test]
    fn test_over_executes() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b": over >r dup r> swap ;").unwrap();
        fe.evaluate(b"1 2 over").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(1));
        assert_eq!(fe.pop().unwrap(), Cell(2));
        assert_eq!(fe.pop().unwrap(), Cell(1));
    }

    #[test]
    fn test_undefined_word() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        assert!(matches!(fe.evaluate(b"nope"), Err(Error::UndefinedWord(_))));
    }

    #[test]
    fn test_constant() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"42 constant foo").unwrap();
        fe.evaluate(b"foo").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
    }

    #[test]
    fn test_variable() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"variable foo").unwrap();
        fe.evaluate(b"42 foo !").unwrap();
        assert_eq!(fe.pop(), Err(Error::Vm(crate::vm::VmError::StackUnderflow)));
        fe.evaluate(b"foo @").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
    }

    #[test]
    fn test_over() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 2 over").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(1));
        assert_eq!(fe.pop().unwrap(), Cell(2));
        assert_eq!(fe.pop().unwrap(), Cell(1));
    }

    #[test]
    fn test_invert() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"0 invert").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(!0usize));
        fe.evaluate(b"-1 invert").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_and() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"10 12 and").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(8));
    }

    #[test]
    fn test_1plus() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"41 1+").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
    }

    #[test]
    fn test_1minus() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"43 1-").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
    }

    #[test]
    fn test_r_fetch() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b": test >r r@ r> drop ;").unwrap();
        fe.evaluate(b"42 test").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
    }

    #[test]
    fn test_xor() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"10 12 xor").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(6));
        fe.evaluate(b"42 0 xor").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
        fe.evaluate(b"42 42 xor").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_minus() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"10 3 -").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(7));
        fe.evaluate(b"0 1 -").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell((-1isize) as usize));
    }

    #[test]
    fn test_plus_store() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"variable x").unwrap();
        fe.evaluate(b"10 x !").unwrap();
        fe.evaluate(b"5 x +!").unwrap();
        fe.evaluate(b"x @").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(15));
    }

    #[test]
    fn test_here() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        let before = fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap();
        fe.evaluate(b"here").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(before));
    }

    #[test]
    fn test_allot() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        let before = fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap();
        fe.evaluate(b"8 allot").unwrap();
        assert_eq!(
            fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap(),
            before + 8
        );
    }

    #[test]
    fn test_cell_plus() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"100 cell+").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(100 + Vm::SIZE));
    }

    #[test]
    fn test_aligned() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"8 aligned").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(8));
        fe.evaluate(b"9 aligned").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(16));
        fe.evaluate(b"15 aligned").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(16));
        fe.evaluate(b"16 aligned").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(16));
    }

    #[test]
    fn test_align() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 allot").unwrap();
        let misaligned = fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap();
        assert_ne!(misaligned % Vm::SIZE, 0);
        fe.evaluate(b"align").unwrap();
        let aligned = fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap();
        assert_eq!(aligned % Vm::SIZE, 0);
        assert!(aligned > misaligned);
    }

    #[test]
    fn test_do_loop() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b": test 0 5 0 do i + loop ;").unwrap();
        fe.evaluate(b"test").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(10)); // 0+1+2+3+4
    }

    #[test]
    fn test_comma() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        let before = fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap();
        fe.evaluate(b"42 ,").unwrap();
        let after = fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap();
        assert_eq!(after, before + Vm::SIZE);
        assert_eq!(fe.data.read_cell(before).unwrap(), 42);
    }
    #[test]
    fn test_comment_paren() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 ( this is ignored ) 2").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(2));
        assert_eq!(fe.pop().unwrap(), Cell(1));
    }

    #[test]
    fn test_comment_backslash() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 \\ ignored").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(1));
    }

    #[test]
    fn test_bl() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"bl").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0x20));
    }

    #[test]
    fn test_true() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"true").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(usize::MAX));
    }

    #[test]
    fn test_false() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"false").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_decimal() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"hex decimal base @").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(10));
    }

    #[test]
    fn test_hex() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"hex base @").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(16));
    }

    #[test]
    fn test_rot() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 2 3 rot").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(1));
        assert_eq!(fe.pop().unwrap(), Cell(3));
        assert_eq!(fe.pop().unwrap(), Cell(2));
    }

    #[test]
    fn test_or() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"10 6 or").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(14));
    }

    #[test]
    fn test_negate() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"42 negate").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell((-42isize) as usize));
        fe.evaluate(b"0 negate").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_equal() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 1 =").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(usize::MAX));
        fe.evaluate(b"1 2 =").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_not_equal() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 2 <>").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(usize::MAX));
        fe.evaluate(b"1 1 <>").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_zero_not_equal() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 0<>").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(usize::MAX));
        fe.evaluate(b"0 0<>").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_less_than() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 2 <").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(usize::MAX));
        fe.evaluate(b"2 1 <").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
        fe.evaluate(b"1 1 <").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_greater_than() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"2 1 >").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(usize::MAX));
        fe.evaluate(b"1 2 >").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
        fe.evaluate(b"1 1 >").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
    }

    #[test]
    fn test_c_comma() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        let before = fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap();
        fe.evaluate(b"$41 c,").unwrap();
        assert_eq!(fe.data.read_char(before).unwrap(), 0x41);
        assert_eq!(
            fe.data.read_cell(fe.layout_addr(Layout::HERE)).unwrap(),
            before + 1
        );
    }

    #[test]
    fn test_body() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"create foo 42 ,").unwrap();
        fe.evaluate(b"' foo >body @").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
    }

    #[test]
    fn test_2dup() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 2 2dup").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(2));
        assert_eq!(fe.pop().unwrap(), Cell(1));
        assert_eq!(fe.pop().unwrap(), Cell(2));
        assert_eq!(fe.pop().unwrap(), Cell(1));
    }

    #[test]
    fn test_2drop() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 2 3 2drop").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(1));
        assert_eq!(fe.pop(), Err(Error::Vm(crate::vm::VmError::StackUnderflow)));
    }

    #[test]
    fn test_2swap() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"1 2 3 4 2swap").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(2));
        assert_eq!(fe.pop().unwrap(), Cell(1));
        assert_eq!(fe.pop().unwrap(), Cell(4));
        assert_eq!(fe.pop().unwrap(), Cell(3));
    }

    #[test]
    fn test_question_dup() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"0 ?dup").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
        assert_eq!(fe.pop(), Err(Error::Vm(crate::vm::VmError::StackUnderflow)));
        fe.evaluate(b"42 ?dup").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
        assert_eq!(fe.pop().unwrap(), Cell(42));
    }

    #[test]
    fn test_abs() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"42 abs").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
        fe.evaluate(b"-42 abs").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(42));
    }

    #[test]
    fn test_min() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"3 5 min").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(3));
        fe.evaluate(b"5 3 min").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(3));
    }

    #[test]
    fn test_max() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"3 5 max").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(5));
        fe.evaluate(b"5 3 max").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(5));
    }

    #[test]
    fn test_s_to_d() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"42 s>d").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0));
        assert_eq!(fe.pop().unwrap(), Cell(42));
        fe.evaluate(b"-1 s>d").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(usize::MAX));
        assert_eq!(fe.pop().unwrap(), Cell(usize::MAX));
    }

    #[test]
    fn test_2store_fetch() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"here 2 cells allot constant pair-addr")
            .unwrap();
        fe.evaluate(b"100 200 pair-addr 2!").unwrap();
        fe.evaluate(b"pair-addr 2@").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(200));
        assert_eq!(fe.pop().unwrap(), Cell(100));
    }

    #[test]
    fn test_move() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"here 3 allot constant src-addr  here 3 allot constant dst-addr")
            .unwrap();
        fe.evaluate(b"$41 src-addr c!  $42 src-addr 1+ c!  $43 src-addr 2 + c!")
            .unwrap();
        fe.evaluate(b"src-addr dst-addr 3 move").unwrap();
        fe.evaluate(b"dst-addr c@  dst-addr 1+ c@  dst-addr 2 + c@")
            .unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(0x43));
        assert_eq!(fe.pop().unwrap(), Cell(0x42));
        assert_eq!(fe.pop().unwrap(), Cell(0x41));
    }

    #[test]
    fn test_s_quote() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b": test s\" hello\" ;").unwrap();
        fe.evaluate(b"test").unwrap();
        let len = usize::from(fe.pop().unwrap());
        let addr = usize::from(fe.pop().unwrap());
        assert_eq!(len, 5);
        assert_eq!(fe.data.read(addr, len).unwrap(), b"hello");
    }

    #[test]
    fn test_count() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"here 2 c, char h c, char i c,").unwrap();
        fe.evaluate(b"here 3 - count").unwrap();
        let len = usize::from(fe.pop().unwrap());
        let addr = usize::from(fe.pop().unwrap());
        assert_eq!(len, 2);
        assert_eq!(fe.data.read(addr, len).unwrap(), b"hi");
    }

    #[test]
    fn test_char() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b"char A").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(b'A' as usize));
    }

    #[test]
    fn test_bracket_char() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b": test [char] Z ;").unwrap();
        fe.evaluate(b"test").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(b'Z' as usize));
    }

    #[test]
    fn test_leave() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();
        fe.evaluate(b": test 0 5 0 do i 3 = if leave then i + loop ;")
            .unwrap();
        fe.evaluate(b"test").unwrap();
        assert_eq!(fe.pop().unwrap(), Cell(3)); // 0+1+2, exits before adding 3
    }

    #[test]
    fn test_environment() {
        let mut fe = TestFe::new([0u8; 65536], NoIo).unwrap();

        let single = |fe: &mut TestFe, q: &[u8], expected: usize| {
            fe.evaluate(q).unwrap();
            assert_eq!(fe.pop().unwrap(), Cell(usize::MAX)); // true
            assert_eq!(fe.pop().unwrap(), Cell(expected));
        };
        let double = |fe: &mut TestFe, q: &[u8], lo: usize, hi: usize| {
            fe.evaluate(q).unwrap();
            assert_eq!(fe.pop().unwrap(), Cell(usize::MAX)); // true
            assert_eq!(fe.pop().unwrap(), Cell(hi));
            assert_eq!(fe.pop().unwrap(), Cell(lo));
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
            size_of::<usize>(),
        );
        single(&mut fe, br#"s" FLOORED" environment?"#, 0);
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
        assert_eq!(fe.pop().unwrap(), Cell(0)); // false
    }
}
