use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use mlua::Lua;
use std::hint::black_box;

macro_rules! bench {
    ($fn_name:ident, $label:literal, $file:literal, $expr:literal) => {
        fn $fn_name(c: &mut Criterion) {
            c.bench_function($label, |b| {
                b.iter_batched(
                    || {
                        let lua = Lua::new();
                        lua.load(include_str!($file)).exec().unwrap();
                        lua
                    },
                    |lua| lua.load(black_box($expr)).exec().unwrap(),
                    BatchSize::SmallInput,
                )
            });
        }
    };
}

bench!(fib, "lua/fib(20)", "fib.lua", "fib(20)");
bench!(
    rangesum,
    "lua/rangesum(1m)",
    "rangesum.lua",
    "rangesum(1000000)"
);
bench!(sieve, "lua/sieve(100)", "sieve.lua", "sieve(100)");
bench!(deepchain, "lua/deepchain", "deepchain.lua", "f5(0)");
bench!(
    countdown,
    "lua/countdown(60)",
    "countdown.lua",
    "countdown(60)"
);
bench!(
    crc32,
    "lua/crc32(4k)",
    "crc32.lua",
    "crc32(string.rep('\\0', 4096))"
);

criterion_group!(benches, fib, rangesum, sieve, deepchain, countdown, crc32);
criterion_main!(benches);
