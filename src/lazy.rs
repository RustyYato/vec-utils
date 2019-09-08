
use std::alloc::Layout;
pub use vec_data::{VecData, Init, Input, Output};
use std::convert::Infallible;
use std::ops::Try;
use std::mem::ManuallyDrop;
use std::marker::PhantomData;

mod vec_data {
    use super::*;

    pub trait State: std::fmt::Debug + Copy + Eq {
        unsafe fn drop_data<T>(v: &mut VecData<T, Self>);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Init;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Input {
        has_output: bool
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Output;

    impl State for Init {
        unsafe fn drop_data<T>(v: &mut VecData<T, Self>) {
            Vec::from_raw_parts(
                v.start,
                v.len,
                v.cap
            );
        }
    }

    impl State for Input {
        unsafe fn drop_data<T>(v: &mut VecData<T, Self>) {
            defer! {
                if !v.state.has_output {
                    Vec::from_raw_parts(
                        v.start, 0, v.cap
                    );
                }
            }

            std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
                v.ptr,
                v.len
            ));
        }
    }

    impl State for Output {
        unsafe fn drop_data<T>(v: &mut VecData<T, Self>) {
            Vec::from_raw_parts(
                v.start,
                v.len,
                v.cap
            );
        }
    }

    pub struct VecData<T, S: State> {
        start: *mut T,
        ptr: *mut T,
        len: usize,
        cap: usize,
        state: S,
        drop: PhantomData<T>
    }

    impl<T, S: State> VecData<T, S> {
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.len == 0
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.len
        }

        #[inline]
        pub fn capacity(&self) -> usize {
            self.cap
        }

        #[inline]
        pub fn state(&self) -> S {
            self.state
        }
    }
    
    impl<T> VecData<T, Input> {
        #[inline]
        pub(crate) unsafe fn next_unchecked(&mut self) -> T {
            debug_assert_ne!(self.len, 0);
            
            let value = self.ptr.read();

            self.ptr = self.ptr.add(1);
            self.len -= 1;

            value
        }

        pub unsafe fn make_output<U>(&mut self) -> VecData<U, Output> {
            debug_assert!(!self.state.has_output);
            
            self.state.has_output = true;

            VecData {
                start: self.start as *mut U,
                ptr: self.ptr as *mut U,
                len: 0,
                cap: self.cap,
                state: Output,
                drop: PhantomData
            }
        }
    }

    impl<T> VecData<T, Init> {
        #[inline]
        pub fn into_vec(self) -> Vec<T> {
            let vec=  ManuallyDrop::new(self);
            
            unsafe {
                Vec::from_raw_parts(
                    vec.start,
                    vec.len,
                    vec.cap
                )
            }
        }

        #[inline]
        pub fn into_input(self) -> VecData<T, Input> {
            let vec = ManuallyDrop::new(self);

            VecData {
                start: vec.start,
                ptr: vec.ptr,
                len: vec.len,
                cap: vec.cap,
                state: Input { has_output: false },
                drop: PhantomData
            }
        }
        
        #[inline]
        pub fn into_output(self) -> VecData<T, Output> {
            let vec = ManuallyDrop::new(self);
            
            VecData {
                start: vec.start,
                ptr: vec.ptr,
                len: 0,
                cap: vec.cap,
                state: Output,
                drop: PhantomData
            }
        }
    }

    impl<T> VecData<T, Output> {
        #[inline]
        pub unsafe fn write_unchecked(&mut self, value: T) {
            debug_assert!(self.len < self.cap);

            self.ptr.write(value);

            self.ptr = self.ptr.add(1);
            self.len += 1;
        }

        #[inline]
        pub fn into_vec(self) -> Vec<T> {
            let vec=  ManuallyDrop::new(self);
            
            unsafe {
                Vec::from_raw_parts(
                    vec.start,
                    vec.len,
                    vec.cap
                )
            }
        }
    }

    impl<T> From<Vec<T>> for VecData<T, Init> {
        fn from(v: Vec<T>) -> Self {
            let mut v = std::mem::ManuallyDrop::new(v);

            Self {
                start: v.as_mut_ptr(),
                ptr: v.as_mut_ptr(),
                len: v.len(),
                state: Init,
                cap: v.capacity(),
                drop: PhantomData
            }
        }
    }

    impl<T> From<Vec<T>> for VecData<T, Input> {
        fn from(v: Vec<T>) -> Self {
            let mut v = std::mem::ManuallyDrop::new(v);

            Self {
                start: v.as_mut_ptr(),
                ptr: v.as_mut_ptr(),
                len: v.len(),
                state: Input {
                    has_output: false
                },
                cap: v.capacity(),
                drop: PhantomData
            }
        }
    }

    impl<T, S: State> Drop for VecData<T, S> {
        fn drop(&mut self) {
            unsafe {
                S::drop_data(self)
            }
        }
    }
}

pub unsafe trait VecTransform {
    type Item;
    type Error;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error>;

    fn is_empty(&self) -> bool;

    fn len(&self) -> usize;

    fn is_compat<U>(&self) -> Option<usize> where Self: Sized;

    unsafe fn make_output<U>(&mut self) -> VecData<U, Output> where Self: Sized;

    #[inline]
    fn map<F: FnMut(Self::Item) -> U, U>(self, func: F) -> Map<Self, F> where Self: Sized {
        Map {
            trans: self,
            func
        }
    }
    
    #[inline]
    fn try_map<F: FnMut(Self::Item) -> R, U, R: Try<Ok = U>>(self, func: F) -> TryMap<Self, F> where Self: Sized {
        TryMap {
            trans: self,
            func
        }
    }

    #[inline]
    fn zip<U: VecTransform>(self, other: U) -> Zip<Self, U> where Self: Sized {
        Zip {
            left: self,
            right: other
        }
    }

    #[inline]
    fn into_writer(self) -> Writer<Self> where Self: Sized {
        Writer::new(self)
    }
}

pub trait IntoInfallible {
    fn into_infallible(self) -> Infallible;
}

impl IntoInfallible for Infallible {
    #[inline]
    fn into_infallible(self) -> Infallible {
        self
    }
}

unsafe impl<T> VecTransform for VecData<T, Input> {
    type Item = T;
    type Error = Infallible;

    #[inline]
    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        Ok(self.next_unchecked())
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn is_compat<U>(&self) -> Option<usize> {
        if Layout::new::<T>() == Layout::new::<U>() {
            Some(self.capacity())
        } else {
            None
        }
    }
    
    unsafe fn make_output<U>(&mut self) -> VecData<U, Output> {
        self.make_output()
    }
}

pub struct Map<T, F> {
    trans: T,
    func: F
}

unsafe impl<T: VecTransform, F: FnMut(T::Item) -> U, U> VecTransform for Map<T, F> {
    type Item = U;
    type Error = T::Error;

    #[inline]
    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        self.trans.next_unchecked().map(&mut self.func)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.trans.is_empty()
    }

    fn len(&self) -> usize {
        self.trans.len()
    }

    fn is_compat<V>(&self) -> Option<usize> {
        self.trans.is_compat::<V>()
    }
    
    unsafe fn make_output<V>(&mut self) -> VecData<V, Output> {
        self.trans.make_output::<V>()
    }
}

pub struct TryMap<T, F> {
    trans: T,
    func: F
}

pub enum Either<T, F> {
    Inner(T),
    Func(F)
}

impl<T: IntoInfallible, F: IntoInfallible> IntoInfallible for Either<T, F> {
    #[inline]
    fn into_infallible(self) -> Infallible {
        match self {
            Either::Inner(x) => x.into_infallible(),
            Either::Func(x) => x.into_infallible(),
        }
    }
}

impl<T: IntoInfallible, F> Either<T, F> {
    #[inline]
    pub fn into_func(self) -> F {
        match self {
            Either::Inner(x) => match x.into_infallible() {},
            Either::Func(x) => x,
        }
    }
}

impl<T, F: IntoInfallible> Either<T, F> {
    #[inline]
    pub fn into_inner(self) -> T {
        match self {
            Either::Inner(x) => x,
            Either::Func(x) => match x.into_infallible() {},
        }
    }
}

unsafe impl<T: VecTransform, F: FnMut(T::Item) -> R, U, R: Try<Ok = U>> VecTransform for TryMap<T, F> {
    type Item = U;
    type Error = Either<T::Error, R::Error>;

    #[inline]
    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        let output = 
            self.trans.next_unchecked().map(&mut self.func).map(R::into_result);

        match output {
            Ok(Ok(value)) => Ok(value),
            Err(err) => Err(Either::Inner(err)),
            Ok(Err(err)) => Err(Either::Func(err)),
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.trans.is_empty()
    }

    fn len(&self) -> usize {
        self.trans.len()
    }

    fn is_compat<V>(&self) -> Option<usize> {
        self.trans.is_compat::<V>()
    }
    
    unsafe fn make_output<V>(&mut self) -> VecData<V, Output> {
        self.trans.make_output::<V>()
    }
}

pub struct Zip<T, U> {
    left: T,
    right: U
}

pub enum ZipError<T, U> {
    Both(T, U),
    Left(T),
    Right(U)
}

impl<T: IntoInfallible, F: IntoInfallible> IntoInfallible for ZipError<T, F> {
    #[inline]
    fn into_infallible(self) -> Infallible {
        match self {
            | ZipError::Both(x, _)
            | ZipError::Left(x) => x.into_infallible(),
            ZipError::Right(x) => x.into_infallible()
        }
    }
}

impl<F> ZipError<Infallible, F> {
    #[inline]
    pub fn into_right(self) -> F {
        match self {
            | ZipError::Both(x, _)
            | ZipError::Left(x) => match x {},
            ZipError::Right(x) => x,
        }
    }
}

impl<T> ZipError<T, Infallible> {
    #[inline]
    pub fn into_left(self) -> T {
        match self {
            | ZipError::Both(_, x)
            | ZipError::Right(x) => match x {},
            ZipError::Left(x) => x,
        }
    }
}

unsafe impl<T: VecTransform, U: VecTransform> VecTransform for Zip<T, U> {
    type Item = (T::Item, U::Item);
    type Error = ZipError<T::Error, U::Error>;

    #[inline]
    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        match (self.left.next_unchecked(), self.right.next_unchecked()) {
            (Ok(left), Ok(right)) => Ok((left, right)),
            (Ok(_), Err(right)) => Err(ZipError::Right(right)),
            (Err(left), Ok(_)) => Err(ZipError::Left(left)),
            (Err(left), Err(right)) => Err(ZipError::Both(left, right)),
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.left.is_empty() || self.right.is_empty()
    }

    fn len(&self) -> usize {
        self.left.len().min(self.right.len())
    }

    fn is_compat<V>(&self) -> Option<usize> {
        match (self.left.is_compat::<V>(), self.right.is_compat::<V>()) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a@Some(_), _) => a,
            (_, a) => a
        }
    }
    
    unsafe fn make_output<V>(&mut self) -> VecData<V, Output> {
        match (self.left.is_compat::<V>(), self.right.is_compat::<V>()) {
            (Some(a), Some(b)) => {
                if a >= b {
                    self.left.make_output()
                } else {
                    self.right.make_output()
                }
            },
            (Some(_), _) => self.left.make_output(),
            (_, Some(_)) => self.right.make_output(),
            (None, None) => {
                debug_assert!(false, "Must be compat for make_output");
                std::hint::unreachable_unchecked()
            }
        }
    }
}

pub struct Writer<V: VecTransform> {
    output: VecData<V::Item, Output>,
    trans: V
}

impl<V: VecTransform> Writer<V> {
    #[inline]
    pub fn new(mut trans: V) -> Self {
        Self {
            output: 
            if trans.is_compat::<V::Item>().is_some() {
                unsafe {
                    trans.make_output()
                }
            } else {
                VecData::from(Vec::with_capacity(trans.len())).into_output()
            }
            ,
            trans
        }
    }

    #[inline]
    pub fn try_into_vec(mut self) -> Result<Vec<V::Item>, V::Error> {
        while !self.trans.is_empty() {
            unsafe {
                let item = self.trans.next_unchecked();

                self.output.write_unchecked(item?);
            }
        }

        Ok(self.output.into_vec())
    }
}

impl<V: VecTransform> Writer<V> where V::Error: IntoInfallible {
    #[inline]
    pub fn into_vec(self) -> Vec<V::Item> {
        match self.try_into_vec() {
            Ok(x) => x,
            Err(x) => match x.into_infallible() {}
        }
    }
}
