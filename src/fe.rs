//! The outer interpreter.
use core::mem::offset_of;

use crate::counted::CountedStr31;
use crate::types::{Double, SignedDouble};
use crate::{Error, FALSE, Result, SIZE, TRUE};

use super::data::{Data, Mem};
use super::io::{Io, NoIo};
use super::parser;
use super::vm::{Op, Stop, Vm};

/// The maximum word length in bytes.
const MAX_WORD_LEN: usize = 31;
/// The maximum number of builtins in the builtins table.
const MAX_BUILTINS: usize = 256;
/// The size of the terminal input buffer.
const INPUT_BUFFER_SIZE: usize = 256;

/// The offset of the `info` field from the `code` field.
const INFO_FROM_CFA: usize = 2 * SIZE;
/// The immediate bitflag.
const IMMEDIATE: u8 = 0b01;
/// The hidden bitflag.
const HIDDEN: u8 = 0b10;

const BL: usize = 0x20;

const CORE: &[u8] = include_bytes!("core.fth");
const CORE_EXT: &[u8] = include_bytes!("core-ext.fth");
const TOOLS: &[u8] = include_bytes!("tools.fth");

pub type Builtin<M, I> = fn(&mut Fe<M, I>) -> Result<()>;

#[derive(Clone, Copy)]
enum Token {
    Lit(usize),
    Name(&'static [u8]),
}

/// System environment configuration.
// TODO: Split this into system invariants (type sizes) and user configuration (buffer sizes and
// stack lengths).
#[derive(Debug, Clone, Copy)]
pub struct Environment {
    /// The maximum length of a counted string (bytes).
    pub counted_string: usize,
    /// The size of the pictured numeric output buffer (bytes).
    pub hold: usize,
    /// The size of the `pad` scratch area (bytes).
    pub pad: usize,
    /// The size of one address unit (bits).
    pub address_unit_bits: usize,
    /// Whether floored division is the default.
    pub floored: bool,
    /// The maximum value of a character (*char*).
    pub max_char: usize,
    /// The maximum value of a signed double.
    pub max_d: SignedDouble,
    /// The maximum value of a signed integer.
    pub max_n: isize,
    /// The maximum value of an unsigned integer.
    pub max_u: usize,
    /// The maximum value of an unsigned double.
    pub max_ud: Double,
    /// The number of cells in the return stack.
    pub return_stack_cells: usize,
    /// The number of cells in the data stack.
    pub stack_cells: usize,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            counted_string: u8::MAX as usize,
            hold: 64,
            pad: 84,
            address_unit_bits: u8::BITS as usize,
            floored: false,
            max_char: u8::MAX as usize,
            max_d: SignedDouble::MAX,
            max_n: isize::MAX,
            max_u: usize::MAX,
            max_ud: Double::MAX,
            return_stack_cells: 64,
            stack_cells: 64,
        }
    }
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
    builtins: [Option<Builtin<M, I>>; MAX_BUILTINS],
    builtins_len: usize,
    layout_base: usize,
    env: Environment,
}

impl<M: Mem, I: Io> Fe<M, I> {
    pub fn new(mem: M, io: I) -> Result<Self> {
        Self::with_env(mem, io, Environment::default())
    }

    pub fn with_env(mem: M, io: I, env: Environment) -> Result<Self> {
        if !Vm::layout_ok(env.stack_cells, env.return_stack_cells) {
            return Err(Error::StacksTooSmall);
        }
        let data = Data::new(mem);
        let vm = Vm::new(env.stack_cells, env.return_stack_cells);
        let layout_base = vm.reserved();
        let mut fe = Self {
            vm,
            data,
            io,
            builtins: [None; MAX_BUILTINS],
            builtins_len: 0,
            op_xts: [0; 256],
            layout_base,
            env,
        };
        fe.bootstrap()?;
        Ok(fe)
    }

    /// Push an item to the data stack.
    ///
    /// ```text
    /// ( -- x )
    /// ```
    pub fn push(&mut self, x: usize) -> Result<()> {
        self.vm.push(&mut self.data, x)?;
        Ok(())
    }

    /// Pop an item from the data stack.
    ///
    /// ```text
    /// ( x --  )
    /// ```
    pub fn pop(&mut self) -> Result<usize> {
        Ok(self.vm.pop(&mut self.data)?)
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
        let (interpret_xt, _) = self.lookup(b"(interpret)")?.unwrap(); // unwrap: temporary
        let mut stop = self.vm.call(&mut self.data, interpret_xt)?;
        loop {
            match stop {
                Stop::Halt => return Ok(()),
                Stop::Yield(token) => {
                    let f = self.builtins[token.index]
                        .ok_or(Error::InvalidBuiltin(token.index as u8))?;
                    if let Err(e) = f(self) {
                        let _ = self.data.write_cell(self.layout_addr(Layout::STATE), FALSE);
                        self.vm.reset();
                        return Err(e);
                    }
                    stop = self.vm.resume(&mut self.data, token)?;
                }
            }
        }
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
    fn bootstrap(&mut self) -> Result<()> {
        self.reserve_variables()?;
        self.compile_opcodes()?;
        self.register_builtins()?;
        self.compile_kernel()?;
        self.define_variables()?;
        self.compile_environment()?;
        self.load_wordlists()
    }

    /// Reserve cells for system variables.
    fn reserve_variables(&mut self) -> Result<()> {
        let variables: &[(usize, usize)] = &[
            (Layout::HERE, self.layout_base + Layout::DATA),
            (Layout::LATEST, 0),
            (Layout::SOURCE_ADDR, self.layout_base + Layout::INPUT),
            (Layout::SOURCE_LEN, 0),
            (Layout::TO_IN, 0),
            (Layout::BASE, 10),
            (Layout::STATE, 0),
            (Layout::SP0, Vm::DS_ADDR),
            (Layout::RP0, self.vm.rs_addr() + SIZE),
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
            (b"dup", Op::Dup),
            (b"r>", Op::RFrom),
            (b"swap", Op::Swap),
            (b"lshift", Op::LShift),
            (b"rshift", Op::RShift),
            (b"um/mod", Op::UmDivMod),
            (b"execute", Op::Execute),
        ];
        for (name, op) in opcodes {
            let xt = self.compile(name, 0, *op, &[])?;
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
        let builtins: &[(&[u8], Builtin<M, I>, u8)] = &[
            (b"'", Self::tick, 0),
            (b"(interpret)", Self::interpret, 0),
            (b"(number)", Self::number, 0),
            (b"emit", Self::emit, 0),
            (b"(find)", Self::find, 0),
            (b"key", Self::key, 0),
            (b"parse", Self::parse, 0),
            (b"postpone", Self::postpone, IMMEDIATE),
            (b"refill", Self::refill, 0),
            (b"(header)", Self::header, 0),
            (b">number", Self::to_number, 0),
            (b"(undefined)", Self::undefined, 0),
            (b"(number?)", Self::numberq, 0),
        ];
        for (name, f, flags) in builtins {
            self.register_builtin(name, *f, *flags)?;
        }
        Ok(())
    }

    /// Compile compiler words.
    #[rustfmt::skip]
    fn compile_kernel(&mut self) -> Result<()> {
        macro_rules! compile {
            ($s:expr, $flags:expr, $code:expr) => {
                self.compile($s, $flags, $code, &[])?;
            };
            ($s:expr, $flags:expr, $code:expr, [$($body:expr),* $(,)?]) => {
                self.compile($s, $flags, $code, &[$($body),*])?;
            };
        }

        macro_rules! addr {
            ($name:ident) => {
                L(self.layout_addr(Layout::$name))
            };
        }

        use Token::{Lit as L, Name as N};

        // This sequence hand-compiles the words `:`, `;`, `create`, and their direct dependencies.
        // This code *is* Forth, just not written as text. Consider it Forth "assembly".
        //
        // `N(name)` compiles a call to a previously defined word. Any reference to an XT that
        // should be a data value at runtime must be a literal (`L(xt)`), not a call.
        let bl = L(BL);

        // : invert ( x1 -- x2 ) dup (nand) ;
        compile!(b"invert", 0, Op::DoCol, [N(b"dup"), N(b"(nand)")]);
        // : or ( x1 x2 -- x3 ) invert swap invert (nand) ;
        compile!(b"or", 0, Op::DoCol, [N(b"invert"), N(b"swap"), N(b"invert"), N(b"(nand)")]);
        // : and ( x1 x2 -- x3 ) (nand) invert ;
        compile!(b"and", 0, Op::DoCol, [N(b"(nand)"), N(b"invert")]);
        // : - ( n1 n2 -- n3 ) invert 1+ + ;
        compile!(b"-", 0, Op::DoCol, [N(b"invert"), L(1), N(b"+"), N(b"+")]);
        // cells
        compile!(b"cells", 0, Op::DoCol, [L(SIZE), N(b"um*"), N(b"drop")]);
        // : r@ ( -- x ) ( R: x -- x ) (rp@) 2 cells - @ ;
        //
        // : +! ( u addr -- ) dup >r @ + r> ! ;
        compile!(b"+!", 0, Op::DoCol, [N(b"dup"), N(b">r"), N(b"@"), N(b"+"), N(b"r>"), N(b"!")]);
        // : allot ( n -- ) (here) +! ;
        compile!(b"allot", 0, Op::DoCol, [addr!(HERE), N(b"+!")]);
        // : aligned ( addr -- a-addr ) 1 cells 1- + 1 cells 1- invert and ;
        compile!(
            b"aligned",
            0,
            Op::DoCol,
            [L(1), N(b"cells"), L(-1isize as usize), N(b"+"), N(b"+"), L(1), N(b"cells"), L(-1isize as usize), N(b"+"), N(b"invert"), N(b"and")]
        );
        // : align ( -- ) here aligned here - allot ;
        compile!(
            b"align",
            0,
            Op::DoCol,
            [addr!(HERE), N(b"@"), N(b"aligned"), addr!(HERE), N(b"@"), N(b"-"), N(b"allot")]
        );
        // : , ( x -- ) align here ! 1 cells allot ;
        compile!(
            b",",
            0,
            Op::DoCol,
            [N(b"align"), addr!(HERE), N(b"@"), N(b"!"), L(1), N(b"cells"), N(b"allot")]
        );

        // : literal ( x -- ) ['] (lit) , , ; immediate
        //
        // The first lit is the (lit) opcode. The second lit is the XT of (lit).
        // At runtime this executes as `(lit) xt`, compiling the XT of (lit) and then the
        // original top of stack.
        compile!(
            b"literal",
            IMMEDIATE,
            Op::DoCol,
            [L(self.op_xt(Op::Lit)), N(b","), N(b",")]
        );

        // Finally, compile the compilation words.

        // : ] true state ! ;
        compile!(b"]", 0, Op::DoCol, [L(TRUE), addr!(STATE), N(b"!")]);
        // : [ false state ! ;
        compile!(b"[", IMMEDIATE, Op::DoCol, [L(FALSE), addr!(STATE), N(b"!")]);

        // : create ( "<spaces>name" -- ) bl parse (header) (docreate) , 0 , ;
        compile!(
            b"create",
            0,
            Op::DoCol,
            [bl, N(b"parse"), N(b"(header)"), L(Op::DoCreate as usize), N(b","), L(0), N(b",")]
        );

        // (hidden-flag)
        compile!(b"(hidden-flag)", 0, Op::DoCol, [L(HIDDEN.into())]);
        // (immediate-flag)
        compile!(b"(immediate-flag)", 0, Op::DoCol, [L(IMMEDIATE.into())]);

        // (flags-addr)
        //
        // Return the address of the flags byte, calculated from the code address (XT).
        compile!(
            b"(flags-addr)",
            0,
            Op::DoCol,
            [L((2 * SIZE).wrapping_neg()), N(b"+"), L(1), N(b"+")]
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
                bl, N(b"parse"), N(b"(header)"),
                addr!(LATEST), N(b"@"), N(b"(flags-addr)"), N(b"dup"), N(b"c@"), N(b"(hidden-flag)"), N(b"or"), N(b"swap"), N(b"c!"),
                L(self.op_xt(Op::DoCol)), N(b"@"), N(b","),
                N(b"]"),
            ]
        );

        // : ;
        //   ['] (exit) ,
        //   \ Store bodylen.
        //   (latest) @ 3 cells - swap 1 cells add here swap - swap !
        //   \ Unset hidden flag.
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
                L(self.op_xt(Op::Exit)), N(b","),
                // Store bodylen.
                addr!(LATEST), N(b"@"),
                N(b"dup"), L((3 * SIZE).wrapping_neg()), N(b"+"),
                N(b"swap"), L(SIZE), N(b"+"),
                addr!(HERE), N(b"@"), N(b"swap"), N(b"-"),
                N(b"swap"), N(b"!"),
                // Unset hidden flag.
                addr!(LATEST), N(b"@"), N(b"(flags-addr)"), N(b"dup"), N(b"c@"), N(b"(hidden-flag)"), N(b"invert"), N(b"and"), N(b"swap"), N(b"c!"),
                N(b"["),
            ]
        );

        Ok(())
    }

    fn compile_environment(&mut self) -> Result<()> {
        let flag = |b: bool| -> usize { if b { TRUE } else { FALSE } };
        self.compile(
            b"(/counted-string)",
            0,
            Op::DoCol,
            &[Token::Lit(self.env.counted_string)],
        )?;
        self.compile(b"(/hold)", 0, Op::DoCol, &[Token::Lit(self.env.hold)])?;
        self.compile(b"(/pad)", 0, Op::DoCol, &[Token::Lit(self.env.pad)])?;
        self.compile(
            b"(address-unit-bits)",
            0,
            Op::DoCol,
            &[Token::Lit(self.env.address_unit_bits)],
        )?;
        self.compile(
            b"(floored)",
            0,
            Op::DoCol,
            &[Token::Lit(flag(self.env.floored))],
        )?;
        self.compile(
            b"(max-char)",
            0,
            Op::DoCol,
            &[Token::Lit(self.env.max_char)],
        )?;
        let (lo, hi): (usize, usize) = Double(self.env.max_d.0 as _).into();
        self.compile(b"(max-d)", 0, Op::DoCol, &[Token::Lit(lo), Token::Lit(hi)])?;
        self.compile(
            b"(max-n)",
            0,
            Op::DoCol,
            &[Token::Lit(self.env.max_n as usize)],
        )?;
        self.compile(b"(max-u)", 0, Op::DoCol, &[Token::Lit(self.env.max_u)])?;
        let (lo, hi): (usize, usize) = self.env.max_ud.into();
        self.compile(b"(max-ud)", 0, Op::DoCol, &[Token::Lit(lo), Token::Lit(hi)])?;
        self.compile(
            b"(return-stack-cells)",
            0,
            Op::DoCol,
            &[Token::Lit(self.env.return_stack_cells)],
        )?;
        self.compile(
            b"(stack-cells)",
            0,
            Op::DoCol,
            &[Token::Lit(self.env.stack_cells)],
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
            (b">in", Layout::TO_IN),
            (b"base", Layout::BASE),
            (b"state", Layout::STATE),
            (b"(sp0)", Layout::SP0),
            (b"(rp0)", Layout::RP0),
        ];
        for (name, offset) in variables {
            self.compile(name, 0, Op::DoCol, &[Token::Lit(self.layout_base + offset)])?;
        }
        Ok(())
    }

    /// Load wordlists.
    ///
    /// With the compiler words bootstrapped, we can bootstrap the rest of the system in Forth.
    fn load_wordlists(&mut self) -> Result<()> {
        for src in &[CORE, CORE_EXT, TOOLS] {
            for line in src.split(|&b| b == b'\n') {
                if !line.is_empty() {
                    self.evaluate(line)?;
                }
            }
        }
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
    /// The `bodylen` field encodes the length of the body in cells.
    ///
    /// The `info` field packs the flags into the least significant byte and the length into the
    /// next byte. It currently reserves two additional bytes of space.
    ///
    /// The `link` field links to the `code` field of the next word in the dictionary.
    ///
    /// The `code` field contains an [`Op`] code. The compiled `body` of the word, if it exists,
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
    /// +---------------------------------------------------------------+
    /// |                            bodylen                            |
    /// +---------------+---------------+-------------------------------+
    /// |  info (len)   | info (flags)  |        info (reserved)        |
    /// +---------------+---------------+-------------------------------+
    /// |                              link                             |
    /// +---------------------------------------------------------------+
    /// |                              code                             |
    /// +---------------------------------------------------------------+
    /// |                              body...                          |
    /// +---------------------------------------------------------------+
    /// ```
    ///
    /// After `(header)` executes, `here` points to the `code` field address.
    fn header(&mut self) -> Result<()> {
        let len = self.pop()?;
        if len > MAX_WORD_LEN {
            return Err(Error::CountedStrTooLong(len));
        }
        let addr = self.pop()?;
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
        let u = self.pop()?;
        let caddr = self.pop()?;
        let hi = self.pop()?;
        let lo = self.pop()?;
        let acc = Double::from((lo, hi));
        let bytes = self.data.read(caddr, u)?;
        // TODO: Check base size.
        let base = self.data.read_cell(self.layout_addr(Layout::BASE))? as u32;
        let (acc, rest) = parser::to_number(acc, bytes, base);
        let len = bytes.len() - rest.len();
        let (lo, hi): (usize, usize) = acc.into();
        let caddr2 = caddr + len;
        let u2 = rest.len();
        self.push(lo)?;
        self.push(hi)?;
        self.push(caddr2)?;
        self.push(u2)
    }

    fn write_header(&mut self, name: &[u8], flags: u8) -> Result<usize> {
        let len: u8 = name
            .len()
            .try_into()
            .map_err(|_| Error::CountedStrTooLong(name.len()))?;
        let latest = self.data.read_cell(self.layout_addr(Layout::LATEST))?;
        let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
        // pad the name so as to always align info
        let pad = (SIZE - ((here + 1 + len as usize) % SIZE)) % SIZE;
        // name
        let nfa = here + pad;
        self.data.write_char(nfa, len)?;
        self.data.write(nfa + 1, name)?;
        // bodylen (0 until ;)
        let body_len = nfa + 1 + len as usize;
        self.data.write_cell(body_len, 0)?;
        // info
        let info = body_len + SIZE;
        self.data.write_cell(info, pack_info(flags, len))?;
        self.data.write_cell(info + SIZE, latest)?;
        // code
        let cfa = info + 2 * SIZE;
        Ok(cfa)
    }

    fn compile(&mut self, name: &[u8], flags: u8, code: Op, body: &[Token]) -> Result<usize> {
        let xt = self.define(name, code, flags)?;
        let lit_xt = self.op_xts[Op::Lit as usize];
        for &token in body {
            match token {
                Token::Lit(v) => {
                    self.comma(lit_xt)?;
                    self.comma(v)?;
                }
                Token::Name(name) => {
                    let xt = self.xt(name)?;
                    self.comma(xt)?;
                }
            }
        }
        if code == Op::DoCol {
            self.comma(self.op_xts[Op::Exit as usize])?;
            let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
            self.data.write_cell(xt - 3 * SIZE, here - (xt + SIZE))?;
        }
        Ok(xt)
    }

    fn comma(&mut self, val: usize) -> Result<()> {
        let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
        self.data.write_cell(here, val)?;
        self.data
            .write_cell(self.layout_addr(Layout::HERE), here + SIZE)?;
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
        self.push(delim as usize)?;
        self.parse()?;
        let u = self.pop()?;
        let caddr = self.pop()?;
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
        let len = self.pop()?;
        let addr = self.pop()?;
        let mut buf = [0u8; MAX_WORD_LEN];
        buf[..len].copy_from_slice(self.data.read(addr, len)?);
        match self.lookup(&buf[..len])? {
            Some((xt, flag)) => {
                self.push(xt)?;
                self.push(flag as usize)
            }
            None => self.push(0),
        }
    }

    fn lookup(&self, name: &[u8]) -> Result<Option<(usize, isize)>> {
        if name.len() > MAX_WORD_LEN {
            return Ok(None);
        }
        let mut xt = self.data.read_cell(self.layout_addr(Layout::LATEST))?;
        while xt != 0 {
            let info = self.data.read_cell(xt - INFO_FROM_CFA)?;
            let flags = (info >> 8) as u8;
            let wlen = info & 0xFF;
            if flags & HIDDEN == 0 && wlen == name.len() {
                let name_at = xt - INFO_FROM_CFA - SIZE - wlen;
                let b = self.data.read(name_at, wlen)?;
                if name.eq_ignore_ascii_case(b) {
                    let flag = if flags & IMMEDIATE != 0 { 1 } else { -1 };
                    return Ok(Some((xt, flag)));
                }
            }
            xt = self.data.read_cell(xt - SIZE)?;
        }
        Ok(None)
    }

    fn xt(&self, name: &[u8]) -> Result<usize> {
        self.lookup(name)?
            .map(|(xt, _)| xt)
            .ok_or_else(|| Error::UndefinedWord(name.try_into().unwrap_or_default()))
    }

    /// ( "<spaces>name" -- xt )
    // TODO: after implementing errors, move this to Forth
    fn tick(&mut self) -> Result<()> {
        let caddr = self.parse_word(BL as u8)?;
        let len = self.data.read_char(caddr)? as usize;
        self.push(caddr + 1)?;
        self.push(len)?;
        self.find()?;
        let flag = self.pop()? as isize;
        if flag == 0 {
            return Err(self.make_undefined(caddr));
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
        let caddr = self.pop()?;
        let len = self.data.read_char(caddr)? as usize;
        let base = self.data.read_cell(self.layout_addr(Layout::BASE))?;

        if let Some(n) = parser::parse_num(self.data.read(caddr + 1, len)?, base as u32) {
            self.push(n)?;
            self.push(1)
        } else {
            self.push(caddr)?;
            self.push(0)
        }
    }

    // TODO: Move this to Forth.
    fn postpone(&mut self) -> Result<()> {
        let caddr = self.parse_word(BL as u8)?;
        let len = self.data.read_char(caddr)? as usize;
        self.push(caddr + 1)?;
        self.push(len)?;
        self.find()?;
        let flag = self.pop()? as isize;
        if flag == 0 {
            return Err(self.make_undefined(caddr));
        }
        let xt = self.pop()?;
        let is_immediate = flag == 1;
        if is_immediate {
            // Compile the XT directly so that the current word *executes* the target when it runs.
            self.comma(xt)
        } else {
            // Compile `(lit) xt ,` so that the current word *compiles* the target when it runs.
            self.comma(self.op_xts[Op::Lit as usize])?;
            self.comma(xt)?;
            let comma_xt = self.xt(b",")?;
            self.comma(comma_xt)
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
            let c_addr = self.parse_word(BL as u8)?;
            if self.data.read_char(c_addr)? == 0 {
                return Ok(());
            }
            let len = self.data.read_char(c_addr)? as usize;
            self.push(c_addr + 1)?;
            self.push(len)?;
            self.find()?;
            let flag = self.pop()? as isize;
            let state = self.data.read_cell(self.layout_addr(Layout::STATE))?;
            if flag != 0 {
                if state == 0 || flag == 1 {
                    self.execute()?;
                } else {
                    let x = self.pop()?;
                    self.comma(x)?;
                }
            } else {
                self.push(c_addr)?;
                self.number()?;
                let ok = self.pop()? as isize;
                let v = self.pop()?;
                if ok == 1 {
                    if state != 0 {
                        self.comma(self.op_xts[Op::Lit as usize])?;
                        self.comma(v)?;
                    } else {
                        self.push(v)?;
                    }
                } else {
                    return Err(self.make_undefined(v));
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
        let delim = self.pop()? as u8;
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
        self.push(src + start)?;
        self.push(len)
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
            Some(c) => self.push(c as usize),
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
        let c = self.pop()? as u8;
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
                self.push(TRUE)?;
                Ok(())
            }
            Ok(None) => {
                self.push(FALSE)?;
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
            .write_cell(self.layout_addr(Layout::HERE), cfa + SIZE)?;
        self.data
            .write_cell(self.layout_addr(Layout::LATEST), cfa)?;
        Ok(cfa)
    }

    fn make_undefined(&self, c_addr: usize) -> Error {
        let len = self.data.read_char(c_addr).unwrap_or(0) as usize;
        // Return an empty name for an invalid address instead of panicking.
        let bytes = self.data.read(c_addr + 1, len).unwrap_or(&[]);
        let name = core::str::from_utf8(bytes)
            .ok()
            .and_then(|s| CountedStr31::try_from(s).ok())
            .unwrap_or_default();
        Error::UndefinedWord(name)
    }

    fn undefined(&mut self) -> Result<()> {
        let len = self.pop()?;
        let caddr = self.pop()?;
        let mut buf = [0u8; MAX_WORD_LEN];
        buf[..len].copy_from_slice(self.data.read(caddr, len)?);
        let name = core::str::from_utf8(&buf[..len])
            .ok()
            .and_then(|s| CountedStr31::try_from(s).ok())
            .unwrap_or_default();
        Err(Error::UndefinedWord(name))
    }

    fn numberq(&mut self) -> Result<()> {
        let len = self.pop()?;
        let caddr = self.pop()?;
        let base = self.data.read_cell(self.layout_addr(Layout::BASE))?;
        if let Some(n) = parser::parse_num(self.data.read(caddr, len)?, base as u32) {
            self.push(n)?;
            self.push(1)
        } else {
            self.push(caddr)?;
            self.push(len)?;
            self.push(0)
        }
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
}
