use super::{Input, Output};

#[doc(hidden)]  
pub use std::ops::Try;
use std::alloc::Layout;

use seal::Seal;
mod seal {
    use super::*;

    pub unsafe trait Seal {
        const LEN: u64;

        type Item;
        type Data;
        type Iter: Iterator<Item = Self::Item>;

        fn into_data(self) -> Self::Data;

        fn remaining_len(&self) -> usize;

        fn into_iterator(self) -> Self::Iter;

        fn check_layout<V>() -> bool;

        fn max_cap<V>(data: &Self::Data, depth: &mut u64) -> Option<usize>;

        unsafe fn take_output<V>(data: &mut Self::Data) -> Output<V>;

        unsafe fn take_output_impl<V>(_: &mut Self::Data, min_cap: u64) -> Output<V>;

        unsafe fn next_unchecked(data: &mut Self::Data) -> Self::Item;

        unsafe fn drop_rest(data: &mut Self::Data, len: usize);
    }
}

/// A specialized const-list for emulating varaidic generics
///
/// To overload what elements can go in this tuple, please use the
/// [`TupleElem`](trait.TupleElem.html) trait
///
/// This is a sealed trait that is not meant to be extended
pub trait Tuple: Seal {}

/// This trait abstracts away elements of the input stream
///
/// # Safety
///
/// * It must be valid to call `next_unchecked` at least `len` times
/// * `len <= capacity`
/// * if `next_unchecked` defers to another `T: TupleElem`, then you should not call `T::next_unchecked` more than once
///     in your own `next_unchecked`
#[allow(clippy::len_without_is_empty)]
pub unsafe trait TupleElem {
    /// The items yielded from this element
    type Item;

    /// The data-segment that `Output<V>` is derived from
    /// and yields `Item`s
    type Data;

    /// An iterator over the items in the collection
    type Iter: Iterator<Item = Self::Item>;

    /// The capacity of the data-segment
    fn capacity(data: &Self::Data) -> usize;

    /// The currently initialized length of the data-segment
    ///
    /// must be less than or equal to the capacity
    fn len(&self) -> usize;

    /// Convert into a raw data-segment
    fn into_data(self) -> Self::Data;

    /// Convert to an iterator if we cannot reuse the data-segment
    fn into_iterator(self) -> Self::Iter;

    /// If this returns `true` if it is safe to call
    /// `take_output`
    fn check_layout<V>() -> bool;

    /// Try and create a new output data-segment, if the output segment
    /// is created, then it owns it's allocation. So you must not deallocate
    /// the allocation backing `Output<V>`
    /// 
    /// # Safety
    /// 
    /// `check_layout::<V>` must return true.
    unsafe fn take_output<V>(data: &mut Self::Data) -> Output<V>;

    /// Get the next_unchecked element
    ///
    /// # Safety
    ///
    /// This must be called *at most* `len` times
    unsafe fn next_unchecked(data: &mut Self::Data) -> Self::Item;

    /// Drop the rest of the buffer and deallocate
    /// if `do_pick` was never called
    ///
    /// # Safety
    ///
    /// This function should only be called once, and 
    /// `data` should not be used again
    unsafe fn drop_rest(data: &mut Self::Data, len: usize);
}

unsafe impl<A: TupleElem> TupleElem for (A,) {
    type Item = A::Item;
    type Data = A::Data;
    type Iter = A::Iter;

    #[inline(always)]
    fn capacity(data: &Self::Data) -> usize {
        A::capacity(data)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        A::len(&self.0)
    }

    #[inline]
    fn into_data(self) -> Self::Data {
        self.0.into_data()
    }

    #[inline]
    fn into_iterator(self) -> Self::Iter {
        self.0.into_iterator()
    }

    #[inline]
    fn check_layout<V>() -> bool {
        A::check_layout::<V>()
    }

    #[inline]
    unsafe fn take_output<V>(data: &mut Self::Data) -> Output<V> {
        A::take_output(data)
    }

    #[inline]
    unsafe fn next_unchecked(data: &mut Self::Data) -> Self::Item {
        A::next_unchecked(data)
    }

    #[inline]
    unsafe fn drop_rest(data: &mut Self::Data, len: usize) {
        A::drop_rest(data, len)
    }
}

unsafe impl<A> TupleElem for Vec<A> {
    type Item = A;
    type Data = Input<A>;
    type Iter = std::vec::IntoIter<A>;

    #[inline(always)]
    fn capacity(data: &Self::Data) -> usize {
        data.cap
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn into_data(self) -> Self::Data {
        Input::from(self)
    }

    #[inline]
    fn into_iterator(self) -> Self::Iter {
        self.into_iter()
    }

    #[inline]
    fn check_layout<V>() -> bool {
        Layout::new::<A>() == Layout::new::<V>()
    }

    #[inline]
    unsafe fn take_output<V>(data: &mut Self::Data) -> Output<V> {
        debug_assert!(Layout::new::<A>() == Layout::new::<V>());

        data.drop_alloc = false;
        Output::new(data.start as *mut V, data.cap)
    }

    #[inline]
    unsafe fn next_unchecked(data: &mut Self::Data) -> Self::Item {
        let ptr = data.ptr;
        data.ptr = data.ptr.add(1);
        ptr.read()
    }

    #[inline]
    unsafe fn drop_rest(data: &mut Self::Data, len: usize) {
        defer! {
            if data.drop_alloc {
                Vec::from_raw_parts(data.start, 0, data.cap);
            }
        }

        std::ptr::drop_in_place(std::slice::from_raw_parts_mut(data.ptr, data.len - len));
    }
}

impl<A: TupleElem> Tuple for (A,) {}
unsafe impl<A: TupleElem> Seal for (A,) {
    const LEN: u64 = 0;

    type Item = A::Item;
    type Data = A::Data;
    type Iter = A::Iter;

    #[inline]
    fn into_data(self) -> Self::Data {
        self.0.into_data()
    }

    #[inline]
    fn into_iterator(self) -> Self::Iter {
        self.0.into_iterator()
    }

    #[inline]
    fn remaining_len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    fn check_layout<V>() -> bool {
        A::check_layout::<V>()
    }

    #[inline]
    fn max_cap<V>(data: &Self::Data, depth: &mut u64) -> Option<usize> {
        if A::check_layout::<V>() {
            *depth = Self::LEN;
            Some(A::capacity(data))
        } else {
            None
        }
    }

    #[inline]
    unsafe fn take_output<V>(data: &mut Self::Data) -> Output<V> {
        A::take_output::<V>(data)
    }

    #[inline]
    unsafe fn take_output_impl<V>(data: &mut Self::Data, depth: u64) -> Output<V> {
        debug_assert_eq!(Self::LEN, depth);
        A::take_output(data)
    }

    #[inline]
    unsafe fn next_unchecked(data: &mut Self::Data) -> Self::Item {
        A::next_unchecked(data)
    }

    #[inline]
    unsafe fn drop_rest(data: &mut Self::Data, len: usize) {
        A::drop_rest(data, len)
    }
}

impl<A: TupleElem, T: Tuple> Tuple for (A, T) {}
unsafe impl<A: TupleElem, T: Seal> Seal for (A, T) {
    const LEN: u64 = T::LEN + 1;

    type Item = (A::Item, T::Item);
    type Data = (A::Data, T::Data);
    type Iter = std::iter::Zip<A::Iter, T::Iter>;

    #[inline]
    fn into_data(self) -> Self::Data {
        (self.0.into_data(), self.1.into_data())
    }

    #[inline]
    fn into_iterator(self) -> Self::Iter {
        self.0.into_iterator().zip(self.1.into_iterator())
    }

    #[inline]
    fn remaining_len(&self) -> usize {
        self.0.len().min(self.1.remaining_len())
    }

    #[inline]
    fn check_layout<V>() -> bool {
        A::check_layout::<V>() || T::check_layout::<V>()
    }

    #[inline]
    fn max_cap<V>((a, rest): &Self::Data, depth: &mut u64) -> Option<usize> {
        let cap_rest = T::max_cap::<V>(rest, depth);

        if A::check_layout::<V>() {
            let cap = A::capacity(a);

            if let Some(cap_rest) = cap_rest {
                if cap_rest > cap {
                    return Some(cap_rest);
                }
            }

            *depth = Self::LEN;
            Some(cap)
        } else {
            cap_rest
        }
    }

    #[inline]
    unsafe fn take_output<V>(data: &mut Self::Data) -> Output<V> {
        let mut depth = 0;
        let val = Self::max_cap::<V>(data, &mut depth);
        debug_assert!(val.is_some());
        Self::take_output_impl(data, depth)
    }

    #[inline]
    unsafe fn take_output_impl<V>((a, rest): &mut Self::Data, depth: u64) -> Output<V> {
        if Self::LEN == depth {
            A::take_output(a)
        } else {
            T::take_output_impl(rest, depth)
        }
    }

    #[inline]
    unsafe fn next_unchecked((vec, rest): &mut Self::Data) -> Self::Item {
        (A::next_unchecked(vec), T::next_unchecked(rest))
    }

    #[inline]
    unsafe fn drop_rest((vec, rest): &mut Self::Data, len: usize) {
        defer! {
            T::drop_rest(rest, len);
        }

        A::drop_rest(vec, len)
    }
}

struct ZipWithIter<V, In: Tuple> {
    // This left buffer is the one that will be reused
    // to write the output into
    output: Output<V>,

    // We will only read from this buffer
    input: In::Data,

    // the initial length of the input
    initial_len: usize,

    // the remaing length of the input
    remaining_len: usize,

    should_free_output: bool,
}

/// Does the work of the `try_zip_with` or `zip_with` macros.
pub fn try_zip_with_impl<R: Try, In: Tuple>(
    input: In,
    f: impl FnMut(In::Item) -> R,
) -> Result<Vec<R::Ok>, R::Error> {
    if In::check_layout::<R::Ok>() {
        let len = input.remaining_len();
        let mut input = input.into_data();

        ZipWithIter::<_, In> {
            output: unsafe { In::take_output::<R::Ok>(&mut input) },
            input,
            initial_len: len,
            remaining_len: len,
            should_free_output: true,
        }
        .try_into_vec(f)
    } else {
        input.into_iterator().map(f).map(R::into_result).collect()
    }
}

impl<V, In: Tuple> ZipWithIter<V, In> {
    pub fn try_into_vec<R: Try<Ok = V>, F: FnMut(In::Item) -> R>(
        mut self,
        mut f: F,
    ) -> Result<Vec<V>, R::Error> {
        // this does a pointer walk and reads from left and right in lock-step
        // then passes those values to the function to be processed
        unsafe {
            while let Some(remaining_len) = self.remaining_len.checked_sub(1) {
                self.remaining_len = remaining_len;

                let input = In::next_unchecked(&mut self.input);

                self.output.ptr.write(f(input)?);
                self.output.ptr = self.output.ptr.add(1);
            }

            // We don't want to drop `self` if dropping the excess elements panics
            // as that could lead to double drops
            self.should_free_output = false;
            
            let (ptr, len, cap) = (
                self.output.start,
                self.initial_len,
                self.output.cap,
            );

            drop(self);

            // create the vector now, so that if we panic in drop, we don't leak it
            Ok(Vec::from_raw_parts(ptr, len, cap))
        }
    }
}

impl<V, In: Tuple> Drop for ZipWithIter<V, In> {
    fn drop(&mut self) {
        let &mut ZipWithIter {
            ref mut output,
            ref mut input,
            should_free_output,
            initial_len,
            remaining_len,
            ..
        } = self;

        let initialized_len = initial_len - remaining_len;

        defer! {
            if should_free_output {
                unsafe {
                    Vec::from_raw_parts(output.start, initialized_len - 1, output.cap);
                }
            }
        }

        unsafe {
            In::drop_rest(input, initialized_len);
        }
    }
}
