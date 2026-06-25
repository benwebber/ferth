use core::num::NonZero;

pub trait State {
    fn throw_xt(&self) -> Option<NonZero<usize>> {
        None
    }
}

pub struct Booting {}
impl State for Booting {}

pub struct Booted {
    pub xt_catch: usize,
    pub xt_interpret: usize,
    pub xt_throw: usize,
}
impl State for Booted {
    fn throw_xt(&self) -> Option<NonZero<usize>> {
        NonZero::new(self.xt_throw)
    }
}

pub struct Loading {}
impl State for Loading {}

pub struct Ready {
    pub xt_quit: usize,
    pub xt_load: usize,
}
impl State for Ready {}
