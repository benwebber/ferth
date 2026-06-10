use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ferth::{Fe, io::NoIo};
use std::hint::black_box;

fn bench_fib(c: &mut Criterion) {
    c.bench_function("fib(20)", |b| {
        b.iter_batched(
            || {
                let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();
                fe.evaluate(include_bytes!("core.fth")).unwrap();
                fe
            },
            |mut fe| fe.evaluate(black_box(b"20 fib drop")).unwrap(),
            BatchSize::SmallInput,
        )
    });
}

fn bench_range_sum(c: &mut Criterion) {
    c.bench_function("range_sum(1_000_000)", |b| {
        b.iter_batched(
            || {
                let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();
                fe.evaluate(include_bytes!("core.fth")).unwrap();
                fe
            },
            |mut fe| fe.evaluate(black_box(b"1000000 range-sum drop")).unwrap(),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_fib, bench_range_sum);
criterion_main!(benches);
