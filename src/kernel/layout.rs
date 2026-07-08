use core::mem::offset_of;

/// The size of the terminal input buffer.
pub(super) const INPUT_BUFFER_SIZE: usize = 256;
/// The size of the transient buffer region.
pub(super) const TRANSIENT_BUFFER_SIZE: usize = 2 * INPUT_BUFFER_SIZE;

/// The layout of the data space.
///
/// Represents the first region of memory after the VM's internal regions, containing system
/// variables such as `here`.
#[repr(C)]
pub(super) struct Layout {
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
    /// The input source.
    ///
    /// String: -1, user input device: 0.
    source_id: usize,
    /// The initial data stack pointer (`(sp0)`).
    sp0: usize,
    /// The initial return stack pointer (`(rp0)`).
    rp0: usize,
    /// The terminal input buffer.
    input: [u8; INPUT_BUFFER_SIZE],
    /// The transient buffer region.
    transient: [u8; TRANSIENT_BUFFER_SIZE],
    /// The address of a buffer containing a diagnostic message.
    diagnostic_addr: usize,
    /// The length of the message in the diagnostic buffer.
    diagnostic_len: usize,
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
    pub const SOURCE_ID: usize = offset_of!(Self, source_id);
    pub const SP0: usize = offset_of!(Self, sp0);
    pub const RP0: usize = offset_of!(Self, rp0);
    pub const INPUT: usize = offset_of!(Self, input);
    pub const TRANSIENT: usize = offset_of!(Self, transient);
    pub const DIAGNOSTIC_ADDR: usize = offset_of!(Self, diagnostic_addr);
    pub const DIAGNOSTIC_LEN: usize = offset_of!(Self, diagnostic_len);
    pub const DATA: usize = size_of::<Self>();
}
