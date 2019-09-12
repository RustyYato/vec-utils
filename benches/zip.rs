#![feature(test)]

extern crate test;

use test::{black_box, Bencher};

use vec_utils::{
    combin::{Data, IntoVecIter},
    VecExt,
};

#[bench]
fn bench_pure(b: &mut Bencher) {
    let x = (0..10000).map(|x| x as f32).collect::<Vec<_>>();
    let y = (0..10000).map(f64::from).collect::<Vec<_>>();

    b.iter(|| {
        for _ in 0..1000 {
            black_box(x.clone());
            black_box(y.clone());
        }
    })
}

#[bench]
fn bench_zip(b: &mut Bencher) {
    let x = (0..10000).map(|x| x as f32).collect::<Vec<_>>();
    let y = (0..10000).map(f64::from).collect::<Vec<_>>();

    b.iter(|| {
        for _ in 0..1000 {
            black_box(x.clone().zip_with(y.clone(), |x, y| f64::from(x) + y));
        }
    })
}

#[bench]
fn bench_zip_macro(b: &mut Bencher) {
    let x = (0..10000).map(|x| x as f32).collect::<Vec<_>>();
    let y = (0..10000).map(f64::from).collect::<Vec<_>>();

    b.iter(|| {
        for _ in 0..1000 {
            black_box(vec_utils::zip_with! {
                (x.clone(), y.clone()), |x, y| f64::from(x) + y
            });
        }
    })
}

#[bench]
fn bench_zip_combin(b: &mut Bencher) {
    let x = (0..10000).map(|x| x as f32).collect::<Vec<_>>();
    let y = (0..10000).map(f64::from).collect::<Vec<_>>();

    b.iter(|| {
        for _ in 0..1000 {
            black_box(
                Data::from(x.clone())
                    .zip(Data::from(y.clone()))
                    .map(|(x, y)| f64::from(x) + y)
                    .into_vec(),
            );
        }
    })
}

#[bench]
fn bench_zip_iter(b: &mut Bencher) {
    let x = (0..10000).map(|x| x as f32).collect::<Vec<_>>();
    let y = (0..10000).map(f64::from).collect::<Vec<_>>();

    b.iter(|| {
        for _ in 0..1000 {
            black_box(
                x.clone()
                    .into_iter()
                    .zip(y.clone())
                    .map(|(x, y)| f64::from(x) + y)
                    .collect::<Vec<_>>(),
            );
        }
    })
}
