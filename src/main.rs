#![feature(try_trait)]

use std::mem::ManuallyDrop;
use std::marker::PhantomData;
use std::ops::Try;

trait VecExt: Sized {
    type T;
    
    fn map<U, F: FnMut(Self::T) -> U>(self, mut f: F) -> Vec<U> {
        use std::convert::Infallible;
        
        match self.try_map(move |x| Ok::<_, Infallible>(f(x))) {
            Ok(x) => x,
            Err(x) => match x {}
        }
    }
    
    fn try_map<U, R: Try<Ok = U>, F: FnMut(Self::T) -> R>(self, f: F) -> Result<Vec<U>, R::Error>;
    
    fn zip_with<U, V, F: FnMut(Self::T, U) -> V>(self, other: Vec<U>, mut f: F) -> Vec<V> {
        use std::convert::Infallible;
        
        match self.try_zip_with(other, move |x, y| Ok::<_, Infallible>(f(x, y))) {
            Ok(x) => x,
            Err(x) => match x {}
        }
    }
    
    fn try_zip_with<U, V, R: Try<Ok = V>, F: FnMut(Self::T, U) -> R>(self, other: Vec<U>, f: F) -> Result<Vec<V>, R::Error>;

    fn drop_and_reuse<U>(self) -> Vec<U>;
}

impl<T> VecExt for Vec<T> {
    type T = T;
    
    fn try_map<U, R: Try<Ok = U>, F: FnMut(Self::T) -> R>(self, f: F) -> Result<Vec<U>, R::Error> {
        use std::alloc::Layout;
        
        if Layout::new::<T>() == Layout::new::<U>() {
            let mut vec = ManuallyDrop::new(self);
            
            let iter = MapIter {
                start: vec.as_mut_ptr() as *mut U,
                ptr: vec.as_mut_ptr(),
                init_len: 0,
                len: vec.len(),
                cap: vec.capacity(),
                drop: PhantomData
            };
            
            iter.try_into_vec(f)
        } else {
            self.into_iter().map(f).map(R::into_result).collect()
        }
    }
    
    fn try_zip_with<U, V, R: Try<Ok = V>, F: FnMut(Self::T, U) -> R>(self, other: Vec<U>, mut f: F) -> Result<Vec<V>, R::Error> {
        use std::alloc::Layout;

        match (
            Layout::new::<T>() == Layout::new::<V>(),
            Layout::new::<U>() == Layout::new::<V>(),
            self.capacity() >= other.capacity()
        ) {
            (true, true, true) | (true, false, _) => {
                let mut vec = ManuallyDrop::new(self);
                let mut right = ManuallyDrop::new(other);

                let left_ptr = vec.as_mut_ptr();
                let right_ptr = right.as_mut_ptr();

                ZipWithIter {
                    init_len: 0,
                    start: left_ptr as *mut V,
                    min_len: vec.len().min(right.len()),
                    drop: PhantomData,

                    left: VecData {
                        start: left_ptr,
                        ptr: left_ptr,
                        cap: vec.capacity(),
                        len: vec.len()
                    },

                    right: VecData {
                        start: right_ptr,
                        ptr: right_ptr,
                        cap: right.capacity(),
                        len: right.len()
                    },
                }.try_into_vec(f)
            },
            (true, true, false) | (false, true, _) => {
                let mut vec = ManuallyDrop::new(other);
                let mut right = ManuallyDrop::new(self);

                let left_ptr = vec.as_mut_ptr();
                let right_ptr = right.as_mut_ptr();

                ZipWithIter {
                    init_len: 0,
                    start: left_ptr as *mut V,
                    min_len: vec.len().min(right.len()),
                    drop: PhantomData,

                    left: VecData {
                        start: left_ptr,
                        ptr: left_ptr,
                        cap: vec.capacity(),
                        len: vec.len()
                    },
                    right: VecData {
                        start: right_ptr,
                        ptr: right_ptr,
                        cap: right.capacity(),
                        len: right.len()
                    },
                }.try_into_vec(move |x, y| f(y, x))
            },
            (false, false, _) => {
                self.into_iter().zip(other.into_iter()).map(move |(x, y)| f(x, y)).map(R::into_result).collect()
            }
        }
    }

    fn drop_and_reuse<U>(mut self) -> Vec<U> {
        self.clear();

        self.map(|_| unsafe { std::hint::unreachable_unchecked() })
    }
}

macro_rules! defer {
    ($($do_work:tt)*) => {
        let _guard = OnDrop(Some(|| { $($do_work)* }));
    }
}

struct OnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        self.0.take().unwrap()()
    }
}

struct MapIter<T, U> {
    start: *mut U,
    ptr: *mut T,
    init_len: usize,
    len: usize,
    cap: usize,
    drop: PhantomData<(T, U)>
}

impl<T, U> MapIter<T, U> {
    fn try_into_vec<R: Try<Ok = U>, F: FnMut(T) -> R>(mut self, mut f: F) -> Result<Vec<U>, R::Error> {
        while self.init_len < self.len {
            unsafe {
                let value = f(self.ptr.read())?;
                
                (self.ptr as *mut U).write(value);
                
                self.ptr = self.ptr.add(1);
                self.init_len += 1;
            }
        }
        
        let vec = ManuallyDrop::new(self);
        
        // we don't want to free the memory
        // which is what dropping this `MapIter` will do
        unsafe {
            Ok(Vec::from_raw_parts(vec.start, vec.len, vec.cap))
        }
    }
}

impl<T, U> Drop for MapIter<T, U> {
    fn drop(&mut self) {
        unsafe {
            defer! {
                Vec::from_raw_parts(
                    self.start,
                    self.init_len,
                    self.cap
                );
            }
            
            // offset by 1 because self.ptr is pointing to
            // memory that was just read from, dropping that
            // would lead to a double free
            std::ptr::drop_in_place(
                std::slice::from_raw_parts_mut(
                    self.ptr.add(1),
                    self.len - self.init_len - 1,
                )
            );
        }
    }
}

struct VecData<T> {
    start: *mut T,
    ptr: *mut T,
    len: usize,
    cap: usize
}

struct ZipWithIter<T, U, V> {
    start: *mut V,

    left: VecData<T>,
    right: VecData<U>,

    init_len: usize,
    min_len: usize,

    drop: PhantomData<(T, U, V)>
}

impl<T, U, V> ZipWithIter<T, U, V> {
    fn try_into_vec<R: Try<Ok = V>, F: FnMut(T, U) -> R>(mut self, mut f: F) -> Result<Vec<V>, R::Error> {
        use std::alloc::Layout;

        debug_assert_eq!(Layout::new::<T>(), Layout::new::<V>());
        
        while self.init_len < self.min_len {
            unsafe {
                let value = f(self.left.ptr.read(), self.right.ptr.read())?;
                
                (self.left.ptr as *mut V).write(value);
                
                self.left.ptr = self.left.ptr.add(1);
                self.right.ptr = self.right.ptr.add(1);

                self.init_len += 1;
            }
        }
        
        let vec = ManuallyDrop::new(self);

        unsafe {
            defer! {
                Vec::from_raw_parts(vec.right.start, 0, vec.right.cap);
            }
            
            defer! {
                std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
                    vec.right.ptr,
                    vec.right.len - vec.min_len
                ));
            }
            
            std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
                vec.left.ptr,
                vec.left.len - vec.min_len
            ));
        }

        unsafe {
            Ok(Vec::from_raw_parts(vec.start, vec.min_len, vec.left.cap))
        }
    }
}

impl<T, U, V> Drop for ZipWithIter<T, U, V> {
    fn drop(&mut self) {
        unsafe {
            defer! {
                Vec::from_raw_parts(self.left.start, 0, self.left.cap);
                Vec::from_raw_parts(self.right.start, 0, self.right.cap);
            }
            
            defer! {
                std::ptr::drop_in_place(std::slice::from_raw_parts_mut(self.right.ptr.add(1), self.right.len - self.init_len - 1));
            }
            
            defer! {
                std::ptr::drop_in_place(std::slice::from_raw_parts_mut(self.left.ptr.add(1), self.left.len - self.init_len - 1));
            }

            std::ptr::drop_in_place(std::slice::from_raw_parts_mut(self.start as *mut V, self.init_len));
        }
    }
}

fn main() {
    #[derive(Clone)]
    struct DropMe(u8);

    impl Drop for DropMe {
        fn drop(&mut self) {
            println!("drop {:b}", self.0)
        }
    }
    
    let left = vec![DropMe(1); 5];
    let mut right = vec![DropMe(2); 5];

    let mut count = 0;
    
    assert!(left.try_zip_with(right, |x, y| if count == 3 {
        None
    } else {
        count += 1;

        Some(DropMe(x.0 + y.0))
    }).is_err());
}
