use super::{Input, Output};

use std::mem::ManuallyDrop;
pub use std::ops::Try;

#[macro_export]
macro_rules! try_zip_with {
    ($($vec:expr),* $(,)? => |$($i:ident),* $(,)?| $($work:tt)*) => {{
        $(let $i = $vec;)*
        
        $crate::tuple::try_zip_with(
            $crate::list!(WRAP $($i),*),
            |$crate::list!(PLACE $($i),*)| $($work)*
        )
    }};
}

#[macro_export]
macro_rules! zip_with {
    ($($vec:expr),+ $(,)? => |$($i:ident),+ $(,)?| $($work:tt)*) => {{
        $crate::tuple::unwrap($crate::try_zip_with!(
            $($vec),+ => |$($i),+|
            Ok::<_, std::convert::Infallible>($($work)*)
        ))
    }};
}

#[macro_export]
macro_rules! list {
    (WRAP $e:ident) => {
        ($e,)
    };
    (PLACE $e:ident) => {
        $e
    };
    ($wrap:ident $e:ident $(, $rest:ident)* $(,)?) => {
        ($e, $crate::list!($wrap $($rest),*))
    };
}

use std::alloc::Layout;

pub fn unwrap<T: Try>(t: T) -> T::Ok where T::Error: Into<std::convert::Infallible> {
    match t.into_result() {
        Ok(x) => x,
        Err(x) => match x.into() {}
    }
}

pub unsafe trait Tuple {
    type Item;
    type Data;

    fn into_data(self) -> Self::Data;

    fn check_pick<V>() -> bool;

    fn pick<V>(data: &mut Self::Data) -> Option<Output<V>>;

    unsafe fn pick_impl<V>(_: &mut Self::Data) -> Option<OutputData<V>>;

    unsafe fn read(data: &mut Self::Data) -> Self::Item;

    unsafe fn drop_rest(data: &mut Self::Data);
}

pub struct OutputData<T> {
    output: Output<T>,
    pick: unsafe fn(*mut ()),
    ptr: *mut ()
}

#[allow(clippy::len_without_is_empty)]
pub unsafe trait TupleElem {
    type Item;
    type Data;
    
    /// The capacity of the data-segment
    fn capacity(data: &Self::Data) -> usize;

    /// The currently initialized length of the data-segment
    /// 
    /// must be less than or equal to the capacity
    fn len(&self) -> usize;

    /// Convert into a raw data-segment
    fn into_data(self) -> Self::Data;

    fn check_pick<V>() -> bool;

    unsafe fn try_pick<V>(data: &mut Self::Data) -> Option<Output<V>>;

    unsafe fn do_pick(_: &mut Self::Data);

    unsafe fn read(data: &mut Self::Data) -> Self::Item;

    unsafe fn drop_rest(data: &mut Self::Data);
}

#[inline]
unsafe fn do_pick_erased<A: TupleElem>(ptr: *mut ()) {
    A::do_pick(&mut *(ptr as *mut A::Data))
}

unsafe impl<A> TupleElem for Vec<A> {
    type Item = A;
    type Data = Input<A>;

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
    fn check_pick<V>() -> bool {
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
    unsafe fn read(data: &mut Self::Data) -> Self::Item {
        let ptr = data.ptr;
        data.ptr = data.ptr.add(1);
        ptr.read()
    }

    #[inline]
    unsafe fn drop_rest(data: &mut Self::Data) {
        defer! {
            if data.drop_alloc {
                Vec::from_raw_parts(data.start, 0, data.cap);
            }
        }

        let offset = data.ptr.offset_from(data.start) as usize;

        std::ptr::drop_in_place(std::slice::from_raw_parts_mut(data.ptr, data.len - offset));
    }
}

unsafe impl<A: TupleElem> Tuple for (A,) {
    type Item = A::Item;
    type Data = A::Data;

    #[inline]
    fn into_data(self) -> Self::Data {
        self.0.into_data()
    }

    #[inline]
    fn check_pick<V>() -> bool {
        A::check_pick::<V>()
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
            ptr: data as *mut A::Data as *mut ()
        })
    }
    
    #[inline]
    unsafe fn read(data: &mut Self::Data) -> Self::Item {
        A::read(data)
    }

    #[inline]
    unsafe fn drop_rest(data: &mut Self::Data) {
        A::drop_rest(data)
    }
}

unsafe impl<A: TupleElem, T: Tuple> Tuple for (A, T) {
    type Item = (A::Item, T::Item);
    type Data = (A::Data, T::Data);

    #[inline]
    fn into_data(self) -> Self::Data {
        (self.0.into_data(), self.1.into_data())
    }

    #[inline]
    fn check_pick<V>() -> bool {
        A::check_pick::<V>() || T::check_pick::<V>()
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
                        return Some(rest_output)
                    }
                }
                
                Some(OutputData {
                    output,
                    pick: do_pick_erased::<A>,
                    ptr: a as *mut _ as *mut ()
                })
            },
        }
    }

    #[inline]
    unsafe fn read((vec, rest): &mut Self::Data) -> Self::Item {
        (A::read(vec), T::read(rest))
    }

    #[inline]
    unsafe fn drop_rest((vec, rest): &mut Self::Data) {
        defer! {
            T::drop_rest(rest);
        }

        A::drop_rest(vec)
    }
}

pub trait InitTuple: Tuple {
    type Iter: Iterator<Item = Self::Item>;

    fn min_len(&self) -> usize;

    fn into_iter(self) -> Self::Iter;
}

impl<A> InitTuple for (Vec<A>,) {
    type Iter = std::vec::IntoIter<A>;

    #[inline]
    fn min_len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    fn into_iter(self) -> Self::Iter {
        self.0.into_iter()
    }
}

impl<Tup: InitTuple, A> InitTuple for (Vec<A>, Tup) {
    type Iter = std::iter::Zip<
        std::vec::IntoIter<A>,
        Tup::Iter
    >;

    #[inline]
    fn min_len(&self) -> usize {
        self.0.len()
            .min(self.1.min_len())
    }

    #[inline]
    fn into_iter(self) -> Self::Iter {
        self.0.into_iter().zip(self.1.into_iter())
    }
}

pub struct ZipWithIter<V, In: Tuple> {
    // This left buffer is the one that will be reused
    // to write the output into
    out: Output<V>,

    // We will only read from this buffer
    //
    // I considered using `std::vec::IntoIter`, but that lead to worse code
    // because LLVM wasn't able to elide the bounds check on the iterator
    input: In::Data,

    // the length of the output that has been written to
    init_len: usize,
    // the length of the vectors that must be traversed
    min_len: usize,
}

pub fn try_zip_with<R: Try, In: InitTuple>(input: In, f: impl FnMut(In::Item) -> R) -> Result<Vec<R::Ok>, R::Error> {
    if In::check_pick::<R::Ok>() {
        let len = input.min_len();
        let mut input = input.into_data();

        match In::pick::<R::Ok>(&mut input) {
            Some(out) => ZipWithIter::<_, In> {
                out,
                input,
                init_len: len,
                min_len: len,
            }.try_into_vec(f),
            None => unsafe {
                std::hint::unreachable_unchecked()
            }
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
                
                let input = In::read(&mut self.input);
                
                self.out.ptr.write(f(input)?);
                self.out.ptr = self.out.ptr.add(1);
            }
        }

        // We don't want to drop `self` if dropping the excess elements panics
        // as that could lead to double drops
        let mut vec = ManuallyDrop::new(self);
        let vec = &mut *vec;
        let output;

        unsafe {
            // create the vector now, so that if we panic in drop, we don't leak it
            output = Vec::from_raw_parts(vec.out.start as *mut V, vec.init_len, vec.out.cap);

            In::drop_rest(&mut vec.input);
        }

        Ok(output)
    }
}

impl<V, In: Tuple> Drop for ZipWithIter<V, In> {
    fn drop(&mut self) {
        let len = self.init_len - self.min_len - 1;
        let out = &mut self.out;

        defer! {
            unsafe {
                Vec::from_raw_parts(out.start, len, out.cap);
            }
        }

        unsafe {
            In::drop_rest(&mut self.input);
        }
    }
}