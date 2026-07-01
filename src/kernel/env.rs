use crate::double::{Double, SignedDouble};

#[derive(Debug, Clone, Copy)]
pub(super) struct Environment {
    /// User configuration.
    pub config: Config,
    /// The maximum length of a counted string (bytes).
    pub counted_string: usize,
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
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            config: Config::default(),
            counted_string: u8::MAX as usize,
            address_unit_bits: u8::BITS as usize,
            floored: false,
            max_char: u8::MAX as usize,
            max_d: SignedDouble::MAX,
            max_n: isize::MAX,
            max_u: usize::MAX,
            max_ud: Double::MAX,
        }
    }
}

/// System environment configuration.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// The size of the pictured numeric output buffer (bytes).
    pub hold: usize,
    /// The size of the `pad` scratch area (bytes).
    pub pad: usize,
    /// The number of cells in the return stack.
    pub return_stack_cells: usize,
    /// The number of cells in the data stack.
    pub stack_cells: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // TODO: Validate minimum size of `/hold` (2*bits+2).
            hold: 2 * (usize::BITS as usize) + 2,
            pad: 84,
            return_stack_cells: 64,
            stack_cells: 64,
        }
    }
}
