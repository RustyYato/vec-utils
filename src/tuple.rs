// use super::VecData;

// use std::marker::PhantomData;
// use std::mem::ManuallyDrop;
// use std::ops::Try;

// pub trait Tuple {
//     type Raw;
//     type Data;

//     fn has_size(size: usize) -> bool;

//     fn convert(raw: Self::Raw) -> Self::Data;

//     unsafe fn read(data: &mut Self::Data) -> Self;

//     unsafe fn drop_rest(data: &mut Self::Data, len: usize);
// }

// impl Tuple for () {
//     type Raw = ();
//     type Data = ();

//     #[inline]
//     fn has_size(_: usize) -> bool { false }

//     #[inline]
//     fn convert((): Self::Raw) -> Self::Data {}
    
//     #[inline]
//     unsafe fn read(data: &mut Self::Data) -> Self {}

//     #[inline]
//     unsafe fn drop_rest((): &mut Self::Data, len: usize) {}
// }

// impl<A, T: Tuple> Tuple for (A, T) {
//     type Raw = (Vec<A>, T::Raw);
//     type Data = (VecData<A>, T::Data);

//     #[inline(always)]
//     fn has_size(size: usize) -> bool {
//         if std::mem::size_of::<A>() == size {
//             true
//         } else {
//             T::has_size(size)
//         }
//     }

//     #[inline]
//     fn convert((vec, rest): Self::Raw) -> Self::Data {
//         (VecData::from(vec), T::convert(rest))
//     }

//     #[inline]
//     unsafe fn read((vec, rest): &mut Self::Data) -> Self {
//         let out = vec.ptr.read();
//         vec.ptr = vec.ptr.add(1);
//         (out, T::read(rest))
//     }

//     #[inline]
//     unsafe fn drop_rest((vec, rest): &mut Self::Data, len: usize) {
//         defer! {
//             Vec::from_raw_parts(vec.start, 0, vec.cap);
//             T::drop_rest(rest, len);
//         }

//         std::ptr::drop_in_place(std::slice::from_raw_parts_mut(vec.ptr, vec.len));
//     }
// }

// pub struct ZipWithIter<T, Tup: Tuple, V> {
//     // This left buffer is the one that will be reused
//     // to write the output into
//     inout: VecData<T>,

//     // We will only read from this buffer
//     //
//     // I considered using `std::vec::IntoIter`, but that lead to worse code
//     // because LLVM wasn't able to elide the bounds check on the iterator
//     rest: Tup::Data,

//     // the length of the output that has been written to
//     init_len: usize,
//     // the length of the vectors that must be traversed
//     min_len: usize,

//     // for drop check
//     drop: PhantomData<V>,
// }

// impl<T, Tup: Tuple, V> ZipWithIter<T, Tup, V> {
//     fn try_into_vec<R: Try<Ok = V>, F: FnMut(T, Tup) -> R>(
//         mut self,
//         mut f: F,
//     ) -> Result<Vec<V>, R::Error> {
//         // this does a pointer walk and reads from left and right in lock-step
//         // then passes those values to the function to be processed
//         while let Some(min_len) = self.min_len.checked_sub(1) {
//             unsafe {
//                 self.min_len = min_len;
                
//                 let out = self.inout.ptr as *mut V;
//                 let left = self.inout.ptr.read();
//                 self.inout.ptr = self.inout.ptr.add(1);
//                 let rest = Tup::read(&mut self.rest);
                

//                 let value = f(left, rest)?;

//                 out.write(value);
//             }
//         }

//         // We don't want to drop `self` if dropping the excess elements panics
//         // as that could lead to double drops
//         let vec = ManuallyDrop::new(self);
//         let output;

//         unsafe {
//             // create the vector now, so that if we panic in drop, we don't leak it
//             output = Vec::from_raw_parts(vec.inout.start as *mut V, vec.init_len, vec.inout.cap);

//             // yay for defers running in reverse order and cleaning up the
//             // old vecs properly

//             // cleans up the right vec
//             defer! {
//                 Vec::from_raw_parts(vec.rest.start, 0, vec.rest.cap);
//             }

//             // drops the remaining elements of the right vec
//             defer! {
//                 Tup::drop_rest(
                    
//                 )
//                 std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
//                     vec.rest.ptr,
//                     vec.rest.len - vec.init_len
//                 ));
//             }

//             // drop the remaining elements of the left vec
//             std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
//                 vec.inout.ptr,
//                 vec.inout.len - vec.init_len
//             ));
//         }

//         Ok(output)
//     }
// }