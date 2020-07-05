use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;

use super::{r#try, Try};

mod general_zip;

pub use general_zip::*;

/// A type that contains useful meta-data about a
/// the Vec<_> that it was created from
pub struct Input<T> {
    // the start of the vec data segment
    start: *mut T,

    // the current position in the vec data segment
    ptr: *mut T,

    // the length of the vec data segment
    len: usize,

    // the capacity of the vec data segment
    cap: usize,

    drop_alloc: bool,
    drop: PhantomData<T>,
}

/// An write only buffer that may overlap with some input buffer
/// this allows reuse of that input buffer to turn it into a
/// `Vec<_>` inside of `tuple::try_into_vec`
pub struct Output<T> {
    // the start of the vec data segment
    start: *mut T,

    // the current position in the vec data segment
    ptr: *mut T,

    // the capacity of the vec data segment
    cap: usize,
    drop: PhantomData<T>,
}

impl<T> Output<T> {
    /// Create a new output buffer, this buffer will own it's data segment
    /// from `start` to `start.add(cap)`
    pub unsafe fn new(start: *mut T, cap: usize) -> Self {
        Self {
            start,
            ptr: start,
            cap,
            drop: PhantomData,
        }
    }
}

impl<T> From<Vec<T>> for Input<T> {
    fn from(vec: Vec<T>) -> Self {
        let mut vec = ManuallyDrop::new(vec);

        let ptr = vec.as_mut_ptr();

        Self {
            start: ptr,
            ptr,
            len: vec.len(),
            cap: vec.capacity(),
            drop_alloc: true,
            drop: PhantomData,
        }
    }
}

/// Extension methods for `Vec<T>`
pub trait VecExt: Sized {
    /// The type that the `Vec<T>` stores
    type T;

    /// Map a vector to another vector, will try and reuse the allocation if the
    /// allocation layouts of the two types match, i.e. if
    /// `std::alloc::Layout::<T>::new() == std::alloc::Layout::<U>::new()`
    /// then the allocation will be reused
    fn map<U, F: FnMut(Self::T) -> U>(self, mut f: F) -> Vec<U> {
        use std::convert::Infallible;

        match self.try_map(move |x| Ok::<_, Infallible>(f(x))) {
            Ok(x) => x,
            Err(x) => match x {},
        }
    }

    /// Map a vector to another vector, will try and reuse the allocation if the
    /// allocation layouts of the two types match, i.e. if
    /// `std::alloc::Layout::<T>::new() == std::alloc::Layout::<U>::new()`
    /// then the allocation will be reused
    ///
    /// The mapping function can be fallible, and on early return, it will drop all previous values,
    /// and the rest of the input vector. Thre error will be returned as a `Result`
    fn try_map<U, R: Try<Ok = U>, F: FnMut(Self::T) -> R>(self, f: F) -> Result<Vec<U>, R::Error>;

    /// Zip a vector to another vector and combine them, the result will be returned,
    /// the allocation will be reused if possible, the larger allocation of the input vectors
    /// will be used if all of `T`, `U`, and `V` have the same allocation layouts.
    fn zip_with<U, V, F: FnMut(Self::T, U) -> V>(self, other: Vec<U>, mut f: F) -> Vec<V> {
        use std::convert::Infallible;

        match self.try_zip_with(other, move |x, y| Ok::<_, Infallible>(f(x, y))) {
            Ok(x) => x,
            Err(x) => match x {},
        }
    }

    /// Zip a vector to another vector and combine them, the result will be returned,
    /// the allocation will be reused if possible, the larger allocation of the input vectors
    /// will be used if all of `T`, `U`, and `V` have the same allocation layouts.
    ///
    /// The mapping function can be fallible, and on early return, it will drop all previous values,
    /// and the rest of the input vectors. Thre error will be returned as a `Result`
    fn try_zip_with<U, V, R: Try<Ok = V>, F: FnMut(Self::T, U) -> R>(
        self,
        other: Vec<U>,
        f: F,
    ) -> Result<Vec<V>, R::Error>;

    /// Drops all of the values in the vector and
    /// create a new vector from it if the layouts are compatible
    ///
    /// if layouts are not compatible, then return `Vec::new()`
    fn drop_and_reuse<U>(self) -> Vec<U>;
}

impl<T> VecExt for Vec<T> {
    type T = T;

    fn try_map<U, R: Try<Ok = U>, F: FnMut(Self::T) -> R>(self, f: F) -> Result<Vec<U>, R::Error> {
        // try_zip_with! { self => |x| { f(x) } }

        if Layout::new::<T>() == Layout::new::<U>() {
            let iter = MapIter {
                init_len: 0,
                data: Input::from(self),
                drop: PhantomData,
            };

            iter.try_into_vec(f)
        } else {
            self.into_iter().map(f).map(R::into_result).collect()
        }
    }

    fn try_zip_with<U, V, R: Try<Ok = V>, F: FnMut(Self::T, U) -> R>(
        self,
        other: Vec<U>,
        mut f: F,
    ) -> Result<Vec<V>, R::Error> {
        // try_zip_with! { self, other => |x, y| { f(x, y) } }

        let len = self.len().min(other.len());
        match (
            Layout::new::<T>() == Layout::new::<V>(),
            Layout::new::<U>() == Layout::new::<V>(),
            self.capacity() >= other.capacity(),
        ) {
            (true, true, true) | (true, false, _) => ZipWithIter {
                init_len: len,
                min_len: len,
                drop: PhantomData,

                left: Input::from(self),
                right: Input::from(other),
            }
            .try_into_vec(f),
            (true, true, false) | (false, true, _) => ZipWithIter {
                init_len: len,
                min_len: len,
                drop: PhantomData,

                left: Input::from(other),
                right: Input::from(self),
            }
            .try_into_vec(move |y, x| f(x, y)),
            (false, false, _) => self
                .into_iter()
                .zip(other.into_iter())
                .map(move |(x, y)| f(x, y))
                .map(R::into_result)
                .collect(),
        }
    }

    fn drop_and_reuse<U>(mut self) -> Vec<U> {
        self.clear();

        // no more elements in the vector
        self.map(|_| unsafe { std::hint::unreachable_unchecked() })
    }
}

struct MapIter<T, U> {
    init_len: usize,

    data: Input<T>,

    // for drop check
    drop: PhantomData<U>,
}

impl<T, U> MapIter<T, U> {
    fn try_into_vec<R: Try<Ok = U>, F: FnMut(T) -> R>(
        mut self,
        mut f: F,
    ) -> Result<Vec<U>, R::Error> {
        // does a pointer walk, easy for LLVM to optimize
        while self.init_len < self.data.len {
            unsafe {
                let value = r#try!(f(self.data.ptr.read()));

                (self.data.ptr as *mut U).write(value);

                self.data.ptr = self.data.ptr.add(1);
                self.init_len += 1;
            }
        }

        let vec = ManuallyDrop::new(self);

        // we don't want to free the memory
        // which is what dropping this `MapIter` will do
        unsafe {
            Ok(Vec::from_raw_parts(
                vec.data.start as *mut U,
                vec.data.len,
                vec.data.cap,
            ))
        }
    }
}

impl<T, U> Drop for MapIter<T, U> {
    fn drop(&mut self) {
        unsafe {
            // destroy the initialized output
            defer! {
                Vec::from_raw_parts(
                    self.data.start as *mut U,
                    self.init_len,
                    self.data.cap
                );
            }

            // offset by 1 because self.ptr is pointing to
            // memory that was just read from, dropping that
            // would lead to a double free
            std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
                self.data.ptr.add(1),
                self.data.len - self.init_len - 1,
            ));
        }
    }
}

// The size of these structures don't matter since they are transient
// So I didn't bother optimizing the size of them, and instead put all the
// useful information I wanted, so that it could be initialized all at once
struct ZipWithIter<T, U, V> {
    // This left buffer is the one that will be reused
    // to write the output into
    left: Input<T>,

    // We will only read from this buffer
    //
    // I considered using `std::vec::IntoIter`, but that lead to worse code
    // because LLVM wasn't able to elide the bounds check on the iterator
    right: Input<U>,

    // the length of the output that has been written to
    init_len: usize,
    // the length of the vectors that must be traversed
    min_len: usize,

    // for drop check
    drop: PhantomData<V>,
}

impl<T, U, V> ZipWithIter<T, U, V> {
    fn try_into_vec<R: Try<Ok = V>, F: FnMut(T, U) -> R>(
        mut self,
        mut f: F,
    ) -> Result<Vec<V>, R::Error> {
        debug_assert_eq!(Layout::new::<T>(), Layout::new::<V>());

        // this does a pointer walk and reads from left and right in lock-step
        // then passes those values to the function to be processed
        while let Some(min_len) = self.min_len.checked_sub(1) {
            unsafe {
                self.min_len = min_len;

                let out = self.left.ptr as *mut V;
                let left = self.left.ptr;
                let right = self.right.ptr;

                self.left.ptr = self.left.ptr.add(1);
                self.right.ptr = self.right.ptr.add(1);

                let value = r#try!(f(left.read(), right.read()));

                out.write(value);
            }
        }

        // We don't want to drop `self` if dropping the excess elements panics
        // as that could lead to double drops
        let vec = ManuallyDrop::new(self);
        let output;

        unsafe {
            // create the vector now, so that if we panic in drop, we don't leak it
            output = Vec::from_raw_parts(vec.left.start as *mut V, vec.init_len, vec.left.cap);

            // yay for defers running in reverse order and cleaning up the
            // old vecs properly

            // cleans up the right vec
            defer! {
                Vec::from_raw_parts(vec.right.start, 0, vec.right.cap);
            }

            // drops the remaining elements of the right vec
            defer! {
                std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
                    vec.right.ptr,
                    vec.right.len - vec.init_len
                ));
            }

            // drop the remaining elements of the left vec
            std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
                vec.left.ptr,
                vec.left.len - vec.init_len,
            ));
        }

        Ok(output)
    }
}

impl<T, U, V> Drop for ZipWithIter<T, U, V> {
    fn drop(&mut self) {
        unsafe {
            let len = self.init_len - self.min_len;

            // This will happen last
            //
            // frees the allocated memory, but does not run destructors
            defer! {
                Vec::from_raw_parts(self.left.start, 0, self.left.cap);
                Vec::from_raw_parts(self.right.start, 0, self.right.cap);
            }

            // The order of the next two defers don't matter for correctness
            //
            // They free the remaining parts of the two input vectors
            defer! {
                std::ptr::drop_in_place(std::slice::from_raw_parts_mut(self.right.ptr, self.right.len - len));
            }

            defer! {
                std::ptr::drop_in_place(std::slice::from_raw_parts_mut(self.left.ptr, self.left.len - len));
            }

            // drop the output that we already calculated
            std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
                self.left.start as *mut V,
                len - 1,
            ));
        }
    }
}
