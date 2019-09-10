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
        let (iter, mut output) = if self.get_cap_for::<Self::Item>().is_some() {
            let (iter, output) = unsafe { self.split_vec_iter() };

            (Either::Left(iter), output)
        } else {
            let len = self.len();
            let iter = self.into_vec_iter();
            let output = data::Data::from(Vec::with_capacity(len)).into_output();
            
            (Either::Right(iter), output)
        };

        unsafe {
            let mut iter = iter;

            iter.try_fold((), |(), x| {
                output.write(x);
                Ok::<_, Infallible>(())
            })
            .map_err(|x| match x {
                Either::Left(x) => x,
                Either::Right(x) => match x {},
            })?;
        }

        Ok(output.into_vec())
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

    fn try_map<U, R: Try<Ok = U>, F>(self, func: F) -> TryMap<Self, F>
    where
        F: FnMut(Self::Item) -> R,
        Self: Sized,
    {
        TryMap { iter: self, func }
    }

    fn zip<U: IntoVecIter>(self, iter: U) -> Zip<Self, U>
    where
        Self: Sized,
    {
        Zip { a: self, b: iter, min_len: 0 }
    }
}

pub enum Either<T, U> {
    Left(T),
    Right(U),
}

impl<T: Into<Infallible>, U: Into<Infallible>> Into<Infallible> for Either<T, U> {
    fn into(self) -> Infallible {
        match self {
            Either::Left(t) => t.into(),
            Either::Right(u) => u.into(),
        }
    }
}

pub unsafe trait VecIter {
    type Item;
    type Error;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error>;

    unsafe fn try_fold<A, R: Try<Ok = A>, F: FnMut(A, Self::Item) -> R>(
        &mut self,
        acc: A,
        f: F,
    ) -> Result<A, Either<Self::Error, R::Error>>;
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

    unsafe fn try_fold<A, R: Try<Ok = A>, G: FnMut(A, Self::Item) -> R>(
        &mut self,
        acc: A,
        mut g: G,
    ) -> Result<A, Either<Self::Error, R::Error>> {
        let Map { iter, func } = self;
        iter.try_fold(acc, move |acc, x| g(acc, func(x)))
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

pub struct TryMap<I, F> {
    iter: I,
    func: F,
}

#[allow(clippy::len_without_is_empty)]
unsafe impl<I: VecIter, F: FnMut(I::Item) -> R, U, R: Try<Ok = U>> VecIter for TryMap<I, F> {
    type Item = U;
    type Error = Either<I::Error, R::Error>;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        let out = self.iter.next_unchecked().map(&mut self.func);

        match out.map(R::into_result) {
            Ok(Ok(x)) => Ok(x),
            Ok(Err(x)) => Err(Either::Right(x)),
            Err(x) => Err(Either::Left(x)),
        }
    }

    unsafe fn try_fold<A, RG: Try<Ok = A>, G: FnMut(A, Self::Item) -> RG>(
        &mut self,
        acc: A,
        mut f: G,
    ) -> Result<A, Either<Self::Error, RG::Error>> {
        let TryMap { iter, func } = self;

        iter.try_fold(acc, |acc, x| {
            let x = func(x).into_result().map_err(Either::Right)?;

            f(acc, x).into_result().map_err(Either::Left)
        })
        .map_err(|x: Either<I::Error, Either<RG::Error, R::Error>>| match x {
            Either::Left(x) => Either::Left(Either::Left(x)),
            Either::Right(Either::Left(x)) => Either::Right(x),
            Either::Right(Either::Right(x)) => Either::Left(Either::Right(x)),
        })
    }
}

#[allow(clippy::len_without_is_empty)]
impl<I: IntoVecIter, F: FnMut(I::Item) -> R, U, R: Try<Ok = U>> IntoVecIter for TryMap<I, F> {
    type Item = U;
    type Error = Either<I::Error, R::Error>;
    type SplitIter = TryMap<I::SplitIter, F>;
    type Iter = TryMap<I::Iter, F>;

    fn len(&self) -> usize {
        self.iter.len()
    }

    fn get_cap_for<T>(&self) -> Option<usize> {
        self.iter.get_cap_for::<T>()
    }

    unsafe fn split_vec_iter<T>(self) -> (Self::SplitIter, data::Data<T, data::Output>) {
        let (iter, out) = self.iter.split_vec_iter();

        (
            TryMap {
                iter,
                func: self.func,
            },
            out,
        )
    }

    fn into_vec_iter(self) -> Self::Iter {
        TryMap {
            iter: self.iter.into_vec_iter(),
            func: self.func,
        }
    }
}

pub struct Zip<A, B> {
    a: A,
    b: B,
    min_len: usize
}

impl<A: IntoVecIter, B: IntoVecIter> Zip<A, B> {
    fn min(&self) -> usize {
        self.a.len().min(self.b.len())
    }
}

impl<A: IntoVecIter, B: IntoVecIter> IntoVecIter for Zip<A, B> {
    type Item = (A::Item, B::Item);
    type Error = Either<A::Error, B::Error>;

    #[allow(clippy::type_complexity)]
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
        let min_len = self.min();

        match (self.a.get_cap_for::<T>(), self.b.get_cap_for::<T>()) {
            (Some(a), Some(b)) => {
                if a >= b {
                    let (a, output) = self.a.split_vec_iter();

                    (
                        Either::Left(Zip {
                            a, min_len,
                            b: self.b.into_vec_iter(),
                        }),
                        output,
                    )
                } else {
                    let (b, output) = self.b.split_vec_iter();

                    (
                        Either::Right(Zip {
                            a: self.a.into_vec_iter(),
                            b, min_len,
                        }),
                        output,
                    )
                }
            }
            (Some(_), None) => {
                let (a, output) = self.a.split_vec_iter();

                (
                    Either::Left(Zip {
                        a, min_len,
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
                        b, min_len,
                    }),
                    output,
                )
            }
            (None, None) => std::hint::unreachable_unchecked(),
        }
    }

    fn into_vec_iter(self) -> Self::Iter {
        Zip {
            min_len: self.min(),
            a: self.a.into_vec_iter(),
            b: self.b.into_vec_iter(),
        }
    }
}

unsafe impl<A: VecIter, B: VecIter> VecIter for Zip<A, B> {
    type Item = (A::Item, B::Item);
    type Error = Either<A::Error, B::Error>;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        let a = self.a.next_unchecked().map_err(Either::Left)?;
        let b = self.b.next_unchecked().map_err(Either::Right)?;

        Ok((a, b))
    }

    unsafe fn try_fold<T, R: Try<Ok = T>, F: FnMut(T, Self::Item) -> R>(
        &mut self,
        mut acc: T,
        mut f: F,
    ) -> Result<T, Either<Self::Error, R::Error>> {
        while let Some(min_len) = self.min_len.checked_sub(1) {
            self.min_len = min_len;

            acc = f(acc, self.next_unchecked().map_err(Either::Left)?).into_result().map_err(Either::Right)?;
        }

        Ok(acc)
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

    unsafe fn try_fold<T, R: Try<Ok = T>, F: FnMut(T, Self::Item) -> R>(
        &mut self,
        acc: T,
        f: F,
    ) -> Result<T, Either<Self::Error, R::Error>> {
        match self {
            Either::Left(a) => a.try_fold(acc, f),
            Either::Right(b) => b.try_fold(acc, f),
        }
    }
}
