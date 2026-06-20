use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ferth::{Config, Fe, io::NoIo};
use std::hint::black_box;

macro_rules! impl_bench {
    ($fn:ident, $label:literal, $file:literal, $expr:expr, $fe:expr) => {
        fn $fn(c: &mut Criterion) {
            c.bench_function($label, |b| {
                b.iter_batched(
                    || {
                        let mut fe = $fe;
                        for line in include_bytes!($file).split(|&b| b == b'\n') {
                            if !line.is_empty() {
                                fe.evaluate(line).unwrap();
                            }
                        }
                        fe
                    },
                    |mut fe| fe.evaluate(black_box($expr)).unwrap(),
                    BatchSize::SmallInput,
                )
            });
        }
    };
}

macro_rules! bench {
    ($fn:ident, $label:literal, $file:literal, $expr:expr) => {
        impl_bench!(
            $fn,
            $label,
            $file,
            $expr,
            Fe::new([0u8; 65536], NoIo).unwrap()
        );
    };
    ($fn:ident, $label:literal, $file:literal, $expr:expr, rs = $rs:literal) => {
        impl_bench!(
            $fn,
            $label,
            $file,
            $expr,
            Fe::with_config(
                [0u8; 65536],
                NoIo,
                Config {
                    return_stack_cells: $rs,
                    ..Default::default()
                },
            )
            .unwrap()
        );
    };
}

bench!(fib, "forth/fib(20)", "fib.fth", b"20 fib drop");
bench!(
    rangesum,
    "forth/rangesum(1m)",
    "rangesum.fth",
    b"1000000 rangesum drop"
);
bench!(sieve, "forth/sieve(100)", "sieve.fth", b"100 sieve drop");
bench!(deepchain, "forth/deepchain", "deepchain.fth", b"0 f5 drop");
bench!(
    countdown,
    "forth/countdown(60)",
    "countdown.fth",
    b"60 countdown",
    rs = 256
);
bench!(
    crc32,
    "forth/crc32(4k)",
    "crc32.fth",
    b"here 4096 erase here 4096 crc32 drop"
);

criterion_group!(benches, fib, rangesum, sieve, deepchain, countdown, crc32);
criterion_main!(benches);
