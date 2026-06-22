pub trait State {}

pub struct Booting {}
impl State for Booting {}

pub struct Booted {
    pub xt_catch: usize,
    pub xt_interpret: usize,
}
impl State for Booted {}

pub struct Loading {}
impl State for Loading {}

pub struct Ready {
    pub xt_quit: usize,
    pub xt_load: usize,
}
impl State for Ready {}
