use super::{Input, Output};

pub use std::ops::Try;

use std::alloc::Layout;

pub fn unwrap<T: Try>(t: T) -> T::Ok
where
    T::Error: Into<std::convert::Infallible>,
{
    match t.into_result() {
        Ok(x) => x,
        Err(x) => match x.into() {},
    }
}

use seal::Seal;
mod seal {
    pub trait Seal {}
}

/// A specialized const-list for emulating varaidic generics
///
/// To overload what elements can go in this tuple, please use the
/// [`TupleElem`](trait.TupleElem.html) trait
///
/// # Safety
///
/// I make no safety guarantees about this trait for it's public api
///
/// i.e. it is only safe to use impls from this crate
pub unsafe trait Tuple: Seal {
    type Item;
    type Data;
    type Iter: Iterator<Item = Self::Item>;

    fn into_data(self) -> Self::Data;

    fn min_len(&self) -> usize;

    fn into_iter(self) -> Self::Iter;

    fn check_pick<V>(&self) -> bool;

    fn pick<V>(data: &mut Self::Data) -> Option<Output<V>>;

    unsafe fn pick_impl<V>(_: &mut Self::Data) -> Option<OutputData<V>>;

    unsafe fn next(data: &mut Self::Data) -> Self::Item;

    unsafe fn drop_rest(data: &mut Self::Data, len: usize);
}

/// An implementation detail of `Tuple::parse_impl` that must be exposed to a public api
pub struct OutputData<T> {
    output: Output<T>,
    pick: unsafe fn(*mut ()),
    ptr: *mut (),
}

/// This trait abstracts away elements of the input stream
///
/// # Safety
///
/// * It must be valid to call `next` at least `len` times
/// * `len <= capacity`
/// * if `next` defers to another `T: TupleElem`, then you should not call `T::next` more than once
///     in your own `next`
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
    fn into_iter(self) -> Self::Iter;

    /// If this returns `true` then `try_pick` should return `Some`
    fn check_pick<V>(&self) -> bool;

    /// Try and create a new output data-segment, but
    /// not commit to the data-segment.
    ///
    /// # Safety
    ///
    /// `try_pick` will try to create an output data-segment
    /// but even if it creates one, it must not commit to it
    /// until `do_pick` is called, for example, you should
    /// still be responsible for deallocating the data-segment
    /// if `do_pick` is *not* called
    unsafe fn try_pick<V>(data: &mut Self::Data) -> Option<Output<V>>;

    /// Commit to the data-segment
    ///
    /// By commiting to the data-segment, you are not allowed to deallocate
    /// the memory buffer that `Output<V>` resides in.
    ///
    /// # Safety
    ///
    /// * `try_pick` must have been called and returned `Some` before `do_pick```
    unsafe fn do_pick(_: &mut Self::Data);

    /// Get the next element
    ///
    /// # Safety
    ///
    /// This must be called *at most* `len` times
    unsafe fn next(data: &mut Self::Data) -> Self::Item;

    /// Drop the rest of the buffer and deallocate
    /// if `do_pick` was never called
    ///
    /// # Safety
    ///
    /// This function should only be called once
    unsafe fn drop_rest(data: &mut Self::Data, len: usize);
}

#[inline]
unsafe fn do_pick_erased<A: TupleElem>(ptr: *mut ()) {
    A::do_pick(&mut *(ptr as *mut A::Data))
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
    fn into_iter(self) -> Self::Iter {
        self.0.into_iter()
    }

    #[inline]
    fn check_pick<V>(&self) -> bool {
        self.0.check_pick::<V>()
    }

    #[inline]
    unsafe fn try_pick<V>(data: &mut Self::Data) -> Option<Output<V>> {
        A::try_pick(data)
    }

    #[inline]
    unsafe fn do_pick(data: &mut Self::Data) {
        A::do_pick(data)
    }

    #[inline]
    unsafe fn next(data: &mut Self::Data) -> Self::Item {
        A::next(data)
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
    fn into_iter(self) -> Self::Iter {
        IntoIterator::into_iter(self)
    }

    #[inline]
    fn check_pick<V>(&self) -> bool {
        Layout::new::<A>() == Layout::new::<V>()
    }

    #[inline]
    unsafe fn try_pick<V>(data: &mut Self::Data) -> Option<Output<V>> {
        if Layout::new::<A>() == Layout::new::<V>() {
            Some(Output::new(data.start as *mut V, data.cap))
        } else {
            None
        }
    }

    #[inline]
    unsafe fn do_pick(data: &mut Self::Data) {
        data.drop_alloc = false;
    }

    #[inline]
    unsafe fn next(data: &mut Self::Data) -> Self::Item {
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

impl<A: TupleElem> Seal for (A,) {}
unsafe impl<A: TupleElem> Tuple for (A,) {
    type Item = A::Item;
    type Data = A::Data;
    type Iter = A::Iter;

    #[inline]
    fn into_data(self) -> Self::Data {
        self.0.into_data()
    }

    #[inline]
    fn into_iter(self) -> Self::Iter {
        self.0.into_iter()
    }

    #[inline]
    fn min_len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    fn check_pick<V>(&self) -> bool {
        self.0.check_pick::<V>()
    }

    #[inline]
    fn pick<V>(data: &mut Self::Data) -> Option<Output<V>> {
        unsafe {
            let output = A::try_pick::<V>(data)?;
            A::do_pick(data);
            Some(output)
        }
    }

    #[inline]
    unsafe fn pick_impl<V>(data: &mut Self::Data) -> Option<OutputData<V>> {
        let output = A::try_pick(data)?;

        Some(OutputData {
            output,
            pick: do_pick_erased::<A>,
            ptr: data as *mut A::Data as *mut (),
        })
    }

    #[inline]
    unsafe fn next(data: &mut Self::Data) -> Self::Item {
        A::next(data)
    }

    #[inline]
    unsafe fn drop_rest(data: &mut Self::Data, len: usize) {
        A::drop_rest(data, len)
    }
}

impl<A: TupleElem, T: Tuple> Seal for (A, T) {}
unsafe impl<A: TupleElem, T: Tuple> Tuple for (A, T) {
    type Item = (A::Item, T::Item);
    type Data = (A::Data, T::Data);
    type Iter = std::iter::Zip<A::Iter, T::Iter>;

    #[inline]
    fn into_data(self) -> Self::Data {
        (self.0.into_data(), self.1.into_data())
    }

    #[inline]
    fn into_iter(self) -> Self::Iter {
        self.0.into_iter().zip(self.1.into_iter())
    }

    #[inline]
    fn min_len(&self) -> usize {
        self.0.len().min(self.1.min_len())
    }

    #[inline]
    fn check_pick<V>(&self) -> bool {
        self.0.check_pick::<V>() || self.1.check_pick::<V>()
    }

    #[inline]
    fn pick<V>(data: &mut Self::Data) -> Option<Output<V>> {
        unsafe {
            Self::pick_impl(data).map(|OutputData { output, pick, ptr }| {
                pick(ptr);

                output
            })
        }
    }

    #[inline]
    unsafe fn pick_impl<V>((a, rest): &mut Self::Data) -> Option<OutputData<V>> {
        let rest_pick = T::pick_impl::<V>(rest);

        match A::try_pick::<V>(a) {
            None => rest_pick,
            Some(output) => {
                if let Some(rest_output) = rest_pick {
                    if output.cap < rest_output.output.cap {
                        return Some(rest_output);
                    }
                }

                Some(OutputData {
                    output,
                    pick: do_pick_erased::<A>,
                    ptr: a as *mut _ as *mut (),
                })
            }
        }
    }

    #[inline]
    unsafe fn next((vec, rest): &mut Self::Data) -> Self::Item {
        (A::next(vec), T::next(rest))
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
    out: Output<V>,

    // We will only read from this buffer
    input: In::Data,

    // the length of the output that has been written to
    init_len: usize,
    // the length of the vectors that must be traversed
    min_len: usize,

    should_free: bool,
}

/// Does the work of the `try_zip_with` or `zip_with` macros.
pub fn try_zip_with<R: Try, In: Tuple>(
    input: In,
    f: impl FnMut(In::Item) -> R,
) -> Result<Vec<R::Ok>, R::Error> {
    if input.check_pick::<R::Ok>() {
        let len = input.min_len();
        let mut input = input.into_data();

        match In::pick::<R::Ok>(&mut input) {
            Some(out) => ZipWithIter::<_, In> {
                out,
                input,
                init_len: len,
                min_len: len,
                should_free: true,
            }
            .try_into_vec(f),
            None => unsafe { std::hint::unreachable_unchecked() },
        }
    } else {
        input.into_iter().map(f).map(R::into_result).collect()
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
            while let Some(min_len) = self.min_len.checked_sub(1) {
                self.min_len = min_len;

                let input = In::next(&mut self.input);

                self.out.ptr.write(f(input)?);
                self.out.ptr = self.out.ptr.add(1);
            }
        }

        // We don't want to drop `self` if dropping the excess elements panics
        // as that could lead to double drops
        self.should_free = false;

        unsafe {
            // create the vector now, so that if we panic in drop, we don't leak it
            Ok(Vec::from_raw_parts(
                self.out.start as *mut V,
                self.init_len,
                self.out.cap,
            ))
        }
    }
}

impl<V, In: Tuple> Drop for ZipWithIter<V, In> {
    fn drop(&mut self) {
        let &mut ZipWithIter {
            ref mut out,
            ref mut input,
            should_free,
            init_len,
            min_len,
            ..
        } = self;

        let len = init_len - min_len;

        defer! {
            if should_free {
                unsafe {
                    Vec::from_raw_parts(out.start, len - 1, out.cap);
                }
            }
        }

        unsafe {
            In::drop_rest(input, len);
        }
    }
}
