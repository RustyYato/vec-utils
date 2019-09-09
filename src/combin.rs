use std::convert::Infallible;
use std::ops::Try;

pub mod data;
pub use data::Data;

#[allow(clippy::len_without_is_empty)]
pub trait IntoVecIter {
    type Item;
    type Error;

    type SplitIter: VecIter<Item = Self::Item, Error = Self::Error>;
    type Iter: VecIter<Item = Self::Item, Error = Self::Error>;

    fn len(&self) -> usize;

    fn get_cap_for<U>(&self) -> Option<usize>;

    unsafe fn split_vec_iter<T>(self) -> (Self::SplitIter, data::Data<T, data::Output>);

    fn into_vec_iter(self) -> Self::Iter;

    fn try_into_vec(self) -> Result<Vec<Self::Item>, Self::Error>
    where
        Self: Sized,
    {
        let len = self.len();

        if self.get_cap_for::<Self::Item>().is_some() {
            let (iter, output) = unsafe { self.split_vec_iter() };

            let output = unsafe {
                iter.try_fold(len, output, |mut acc, x| {
                    acc.write(x);
                    Ok(acc)
                })?
            };

            Ok(output.into_vec())
        } else {
            let iter = self.into_vec_iter();
            let output = data::Data::from(Vec::with_capacity(len)).into_output();

            let output = unsafe {
                iter.try_fold(len, output, |mut acc, x| {
                    acc.write(x);
                    Ok(acc)
                })?
            };

            Ok(output.into_vec())
        }
    }

    fn into_vec(self) -> Vec<Self::Item>
    where
        Self::Error: Into<Infallible>,
        Self: Sized,
    {
        match self.try_into_vec() {
            Ok(vec) => vec,
            Err(e) => match e.into() {},
        }
    }

    fn map<U, F>(self, func: F) -> Map<Self, F>
    where
        F: FnMut(Self::Item) -> U,
        Self: Sized,
    {
        Map { iter: self, func }
    }

    fn zip<U: IntoVecIter>(self, iter: U) -> Zip<Self, U>
    where
        Self: Sized,
    {
        Zip { a: self, b: iter }
    }
}

pub enum Either<T, U> {
    Left(T),
    Right(U),
}

pub unsafe trait VecIter {
    type Item;
    type Error;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error>;

    unsafe fn try_fold<A, R: Try<Ok = A, Error = Self::Error>, F: FnMut(A, Self::Item) -> R>(
        mut self,
        len: usize,
        mut acc: A,
        mut f: F,
    ) -> Result<A, Self::Error>
    where
        Self: Sized,
    {
        for _ in 0..len {
            let value = self.next_unchecked()?;
            acc = f(acc, value).into_result()?;
        }

        Ok(acc)
    }
}

pub struct Map<I, F> {
    iter: I,
    func: F,
}

#[allow(clippy::len_without_is_empty)]
unsafe impl<I: VecIter, F: FnMut(I::Item) -> U, U> VecIter for Map<I, F> {
    type Item = U;
    type Error = I::Error;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        self.iter.next_unchecked().map(&mut self.func)
    }

    unsafe fn try_fold<A, R: Try<Ok = A, Error = Self::Error>, G: FnMut(A, Self::Item) -> R>(
        self,
        len: usize,
        acc: A,
        mut g: G,
    ) -> Result<A, Self::Error>
    where
        Self: Sized,
    {
        let Map { iter, mut func } = self;
        iter.try_fold(len, acc, move |acc, x| g(acc, func(x)))
    }
}

#[allow(clippy::len_without_is_empty)]
impl<I: IntoVecIter, F: FnMut(I::Item) -> U, U> IntoVecIter for Map<I, F> {
    type Item = U;
    type Error = I::Error;
    type SplitIter = Map<I::SplitIter, F>;
    type Iter = Map<I::Iter, F>;

    fn len(&self) -> usize {
        self.iter.len()
    }

    fn get_cap_for<T>(&self) -> Option<usize> {
        self.iter.get_cap_for::<T>()
    }

    unsafe fn split_vec_iter<T>(self) -> (Self::SplitIter, data::Data<T, data::Output>) {
        let (iter, out) = self.iter.split_vec_iter();

        (
            Map {
                iter,
                func: self.func,
            },
            out,
        )
    }

    fn into_vec_iter(self) -> Self::Iter {
        Map {
            iter: self.iter.into_vec_iter(),
            func: self.func,
        }
    }
}

pub struct Zip<A, B> {
    a: A,
    b: B,
}

pub enum ZipError<A, B> {
    Both(A, B),
    Left(A),
    Right(B),
}

impl<A: Into<Infallible>, B: Into<Infallible>> Into<Infallible> for ZipError<A, B> {
    fn into(self) -> Infallible {
        match self {
            ZipError::Both(a, _) | ZipError::Left(a) => a.into(),
            ZipError::Right(b) => b.into(),
        }
    }
}

impl<A: Into<Infallible>, B> ZipError<A, B> {
    pub fn into_right(self) -> B {
        match self {
            ZipError::Both(a, _) | ZipError::Left(a) => match a.into() {},
            ZipError::Right(b) => b,
        }
    }
}

impl<A, B: Into<Infallible>> ZipError<A, B> {
    pub fn into_left(self) -> A {
        match self {
            ZipError::Both(_, b) | ZipError::Right(b) => match b.into() {},
            ZipError::Left(a) => a,
        }
    }
}

impl<A: IntoVecIter, B: IntoVecIter> IntoVecIter for Zip<A, B> {
    type Item = (A::Item, B::Item);
    type Error = ZipError<A::Error, B::Error>;

    type SplitIter = Either<Zip<A::SplitIter, B::Iter>, Zip<A::Iter, B::SplitIter>>;

    type Iter = Zip<A::Iter, B::Iter>;

    fn len(&self) -> usize {
        self.a.len().min(self.b.len())
    }

    fn get_cap_for<U>(&self) -> Option<usize> {
        match (self.a.get_cap_for::<U>(), self.b.get_cap_for::<U>()) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        }
    }

    unsafe fn split_vec_iter<T>(self) -> (Self::SplitIter, data::Data<T, data::Output>) {
        match (self.a.get_cap_for::<T>(), self.b.get_cap_for::<T>()) {
            (Some(a), Some(b)) => {
                if a >= b {
                    let (a, output) = self.a.split_vec_iter();

                    (
                        Either::Left(Zip {
                            a,
                            b: self.b.into_vec_iter(),
                        }),
                        output,
                    )
                } else {
                    let (b, output) = self.b.split_vec_iter();

                    (
                        Either::Right(Zip {
                            a: self.a.into_vec_iter(),
                            b,
                        }),
                        output,
                    )
                }
            }
            (Some(_), None) => {
                let (a, output) = self.a.split_vec_iter();

                (
                    Either::Left(Zip {
                        a,
                        b: self.b.into_vec_iter(),
                    }),
                    output,
                )
            }
            (None, Some(_)) => {
                let (b, output) = self.b.split_vec_iter();

                (
                    Either::Right(Zip {
                        a: self.a.into_vec_iter(),
                        b,
                    }),
                    output,
                )
            }
            (None, None) => std::hint::unreachable_unchecked(),
        }
    }

    fn into_vec_iter(self) -> Self::Iter {
        Zip {
            a: self.a.into_vec_iter(),
            b: self.b.into_vec_iter(),
        }
    }
}

unsafe impl<A: VecIter, B: VecIter> VecIter for Zip<A, B> {
    type Item = (A::Item, B::Item);
    type Error = ZipError<A::Error, B::Error>;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        match (self.a.next_unchecked(), self.b.next_unchecked()) {
            (Ok(a), Ok(b)) => Ok((a, b)),
            (Err(a), Ok(_)) => Err(ZipError::Left(a)),
            (Ok(_), Err(b)) => Err(ZipError::Right(b)),
            (Err(a), Err(b)) => Err(ZipError::Both(a, b)),
        }
    }
}

unsafe impl<A: VecIter, B: VecIter<Item = A::Item, Error = A::Error>> VecIter for Either<A, B> {
    type Item = A::Item;
    type Error = A::Error;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        match self {
            Either::Left(a) => a.next_unchecked(),
            Either::Right(b) => b.next_unchecked(),
        }
    }

    unsafe fn try_fold<T, R: Try<Ok = T, Error = Self::Error>, G: FnMut(T, Self::Item) -> R>(
        self,
        len: usize,
        acc: T,
        mut g: G,
    ) -> Result<T, Self::Error>
    where
        Self: Sized,
    {
        match self {
            Either::Left(a) => a.try_fold(len, acc, move |acc, x| g(acc, x)),
            Either::Right(b) => b.try_fold(len, acc, move |acc, x| g(acc, x)),
        }
    }
}
