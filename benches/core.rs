use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ferth::{Fe, io::NoIo};

const FIB: &[u8] = b": fib dup 2 < if exit then dup 1- recurse swap 1- 1- recurse + ;";

fn bench_fib(c: &mut Criterion) {
    c.bench_function("fib(20)", |b| {
        b.iter_batched(
            || {
                let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();
                fe.evaluate(FIB).unwrap();
                fe
            },
            |mut fe| fe.evaluate(black_box(b"20 fib drop")).unwrap(),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_fib);
criterion_main!(benches);
