use criterion::{black_box, criterion_group, criterion_main, Criterion};

use vec_utils::VecExt;

fn benchmark_pure(c: &mut Criterion) {
    let x = (0..20).map(|x| x as f32).collect::<Vec<_>>();
    let y = (0..20).map(f64::from).collect::<Vec<_>>();

    c.bench_function("pure single", |b| b.iter(|| (black_box(x.clone()))));
    c.bench_function("pure double", |b| {
        b.iter(|| (black_box(x.clone()), black_box(y.clone())))
    });
}

fn benchmark_zip(c: &mut Criterion) {
    fn fib2(x: u128, y: u128) -> u128 {
        match x {
            0 | 1 => x + y,
            x => fib2(x - 1, y) + fib2(x - 2, y),
        }
    }

    let x = (0..20).collect::<Vec<_>>();
    let y = (0..20).collect::<Vec<_>>();

    c.bench_function("zip", |b| {
        b.iter(|| black_box(x.clone().zip_with(y.clone(), fib2)))
    });
    c.bench_function("zip macro", |b| {
        b.iter(|| {
            black_box(vec_utils::zip_with! {
                (x.clone(), y.clone()), |x, y| fib2(x, y)
            });
        })
    });
    c.bench_function("zip iter", |b| {
        b.iter(|| {
            x.clone()
                .into_iter()
                .zip(y.clone())
                .map(|(x, y)| fib2(x, y))
                .collect::<Vec<_>>()
        })
    });
}

fn benchmark_map(c: &mut Criterion) {
    fn fib(x: u128) -> u128 {
        match x {
            0 | 1 => x,
            x => fib(x - 1) + fib(x - 2),
        }
    }

    let x = (0..20).map(|x| x).collect::<Vec<_>>();

    c.bench_function("map", |b| b.iter(|| black_box(x.clone().map(fib))));
    c.bench_function("map macro", |b| {
        b.iter(|| {
            black_box(vec_utils::zip_with! {
                x.clone(), |x| fib(x)
            });
        })
    });
    c.bench_function("map iter", |b| {
        b.iter(|| x.clone().into_iter().map(fib).collect::<Vec<_>>())
    });
}

criterion_group! { vec_utils, benchmark_pure, benchmark_map, benchmark_zip }
criterion_main! { vec_utils }
