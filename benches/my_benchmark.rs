use std::{fs, hint::black_box};
use criterion::{criterion_group, criterion_main, Criterion};
use bitcut::make_diff;


fn criterion_benchmark(c: &mut Criterion) {
    let old = fs::read("/Users/kitaev/projects/tochka/fds-rs/300990012_new.cbor").unwrap();
    let new = fs::read("/Users/kitaev/projects/tochka/fds-rs/300990012_newnew.cbor").unwrap();
    c.bench_function("make_diff", |b| b.iter(|| make_diff(black_box(&old), black_box(&new))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);