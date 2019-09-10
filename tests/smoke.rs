use vec_utils::{
    combin::{Data, IntoVecIter},
    VecExt,
};

#[test]
fn map() {
    let vec = vec![0.0f32, 1.0, 2.0, 3.0];

    let vec: Vec<u32> = vec.map(move |x| unsafe { std::mem::transmute(x) });

    assert_eq!(
        vec,
        [
            0.0_f32.to_bits(),
            1.0_f32.to_bits(),
            2.0_f32.to_bits(),
            3.0_f32.to_bits()
        ]
    )
}

#[test]
fn map_combin() {
    let vec = vec![0.0f32, 1.0, 2.0, 3.0];

    let vec: Vec<u32> = Data::from(vec)
        .map(move |x| unsafe { std::mem::transmute(x) })
        .into_vec();

    assert_eq!(
        vec,
        [
            0.0_f32.to_bits(),
            1.0_f32.to_bits(),
            2.0_f32.to_bits(),
            3.0_f32.to_bits()
        ]
    )
}

#[test]
fn zip_with() {
    let a = vec![0.0f32, 1.0, 2.0, 3.0];
    let b = vec![0.0f32, 1.0, 2.0, 3.0];

    let vec: Vec<f32> = a.zip_with(b, move |a, b| a + b);

    assert_eq!(vec, [0.0, 2.0, 4.0, 6.0], "f32 + f32 failed!");

    let a = vec![0.0f64, 1.0, 2.0, 3.0];
    let b = vec![0.0f32, 1.0, 2.0, 3.0];

    let vec: Vec<f64> = a.zip_with(b, move |a, b| a + f64::from(b));

    assert_eq!(vec, [0.0, 2.0, 4.0, 6.0], "f64 + f32 failed!");

    let a = vec![0.0f64, 1.0, 2.0, 3.0];
    let b = vec![0.0f32, 1.0, 2.0, 3.0];

    let vec: Vec<f64> = b.zip_with(a, move |a, b| f64::from(a) + b);

    assert_eq!(vec, [0.0, 2.0, 4.0, 6.0], "f32 + f64 failed!");
}

#[test]
fn zip() {
    let a = vec![0.0f32, 1.0, 2.0, 3.0];
    let b = vec![0.0f32, 1.0, 2.0, 3.0];

    let vec: Vec<f32> = Data::from(a)
        .zip(Data::from(b))
        .map(move |(a, b)| a + b)
        .into_vec();

    assert_eq!(vec, [0.0, 2.0, 4.0, 6.0]);

    let a = vec![0.0f64, 1.0, 2.0, 3.0];
    let b = vec![0.0f32, 1.0, 2.0, 3.0];

    let vec: Vec<f64> = Data::from(a)
        .zip(Data::from(b))
        .map(move |(a, b)| a + f64::from(b))
        .into_vec();

    assert_eq!(vec, [0.0, 2.0, 4.0, 6.0]);

    let a = vec![0.0f32, 1.0, 2.0, 3.0];
    let b = vec![0.0f64, 1.0, 2.0, 3.0];

    let vec: Vec<f64> = Data::from(a)
        .zip(Data::from(b))
        .map(move |(a, b)| f64::from(a) + b)
        .into_vec();

    assert_eq!(vec, [0.0, 2.0, 4.0, 6.0]);
}
