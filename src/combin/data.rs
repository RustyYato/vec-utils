use super::{Either, IntoVecIter, VecIter};

use std::convert::Infallible;
use std::ops::Try;

pub trait State: Sized {
    unsafe fn drop<T>(v: &mut Data<T, Self>);
}

pub unsafe trait InputState: State {}

pub struct Init;

pub struct Input;

pub struct Split;

pub struct Output;

impl State for Init {
    unsafe fn drop<T>(v: &mut Data<T, Self>) {
        let v = &mut v.raw;

        Vec::from_raw_parts(v.start, v.len, v.cap);
    }
}

impl State for Output {
    unsafe fn drop<T>(v: &mut Data<T, Self>) {
        let v = &mut v.raw;

        Vec::from_raw_parts(v.start, v.len, v.cap);
    }
}

unsafe impl InputState for Input {}
impl State for Input {
    unsafe fn drop<T>(v: &mut Data<T, Self>) {
        let v = &mut v.raw;

        defer! {
            Vec::from_raw_parts(
                v.start,
                0,
                v.cap
            );
        }

        std::ptr::drop_in_place(std::slice::from_raw_parts_mut(v.ptr, v.len));
    }
}

unsafe impl InputState for Split {}
impl State for Split {
    unsafe fn drop<T>(v: &mut Data<T, Self>) {
        let v = &mut v.raw;

        std::ptr::drop_in_place(std::slice::from_raw_parts_mut(v.ptr, v.len));
    }
}

struct RawData<T> {
    start: *mut T,
    ptr: *mut T,
    len: usize,
    cap: usize,
}

pub struct Data<T, S: State> {
    raw: RawData<T>,
    _state: S,
}

impl<T, S: State> Drop for Data<T, S> {
    fn drop(&mut self) {
        unsafe {
            S::drop(self);
        }
    }
}

impl<T, S: State> Data<T, S> {
    pub fn len(&self) -> usize {
        self.raw.len
    }

    pub fn is_empty(&self) -> bool {
        self.raw.len == 0
    }
}

impl<T> Data<T, Output> {
    pub unsafe fn write(&mut self, value: T) {
        debug_assert!(self.raw.len < self.raw.cap);

        self.raw.ptr.write(value);
        self.raw.ptr = self.raw.ptr.add(1);
        self.raw.len += 1;
    }

    pub fn into_vec(self) -> Vec<T> {
        let v = &mut std::mem::ManuallyDrop::new(self).raw;

        unsafe { Vec::from_raw_parts(v.start, v.len, v.cap) }
    }
}

impl<T> Data<T, Init> {
    fn into_raw(self) -> RawData<T> {
        unsafe {
            let data = std::mem::ManuallyDrop::new(self);

            std::ptr::read(&data.raw)
        }
    }

    pub fn into_output(self) -> Data<T, Output> {
        let mut raw = self.into_raw();

        raw.len = 0;

        Data {
            raw,
            _state: Output,
        }
    }
}

impl<T> From<Vec<T>> for Data<T, Init> {
    fn from(v: Vec<T>) -> Self {
        let mut v = std::mem::ManuallyDrop::new(v);
        Self {
            raw: RawData {
                start: v.as_mut_ptr(),
                ptr: v.as_mut_ptr(),
                len: v.len(),
                cap: v.capacity(),
            },
            _state: Init,
        }
    }
}

impl<T> IntoVecIter for Data<T, Init> {
    type Item = T;
    type Error = Infallible;
    type SplitIter = Data<T, Split>;
    type Iter = Data<T, Input>;

    fn len(&self) -> usize {
        self.raw.len
    }

    fn get_cap_for<U>(&self) -> Option<usize> {
        use std::alloc::Layout;

        if Layout::new::<T>() == Layout::new::<U>() {
            Some(self.raw.cap)
        } else {
            None
        }
    }

    unsafe fn split_vec_iter<U>(self) -> (Self::SplitIter, Data<U, Output>) {
        let raw = self.into_raw();
        (
            Data {
                raw: RawData { ..raw },
                _state: Split,
            },
            Data {
                raw: RawData {
                    start: raw.start as *mut U,
                    ptr: raw.ptr as *mut U,
                    len: 0,
                    cap: raw.cap,
                },
                _state: Output,
            },
        )
    }

    fn into_vec_iter(self) -> Self::Iter {
        Data {
            raw: self.into_raw(),
            _state: Input,
        }
    }
}

unsafe impl<T, S: InputState> VecIter for Data<T, S> {
    type Item = T;
    type Error = Infallible;

    unsafe fn next_unchecked(&mut self) -> Result<Self::Item, Self::Error> {
        debug_assert!(self.raw.len > 0);

        let value = self.raw.ptr.read();

        self.raw.ptr = self.raw.ptr.add(1);
        self.raw.len -= 1;

        Ok(value)
    }

    unsafe fn try_fold<A, R: Try<Ok = A>, F: FnMut(A, Self::Item) -> R>(
        &mut self,
        mut acc: A,
        mut f: F,
    ) -> Result<A, Either<Self::Error, R::Error>>
    where
        Self: Sized,
    {
        while let Some(len) = self.raw.len.checked_sub(1) {
            self.raw.len = len;
            let ptr = self.raw.ptr;
            self.raw.ptr = self.raw.ptr.add(1);

            acc = f(acc, ptr.read()).into_result().map_err(Either::Right)?;
        }

        Ok(acc)
    }
}
