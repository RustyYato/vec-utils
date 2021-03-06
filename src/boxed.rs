use std::alloc::Layout;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;

/// Extension methods for `Box<T>`
pub trait BoxExt: Sized {
    /// The type that the `Box<T>` stores
    type T: ?Sized;

    /// drops the value inside the box and returns the allocation
    /// in the form of an `UninitBox`
    fn drop_box(bx: Self) -> UninitBox;

    /// takes the value inside the box and returns it as well as the
    /// allocation in the form of an `UninitBox`
    fn take_box(bx: Self) -> (UninitBox, Self::T)
    where
        Self::T: Sized;
}

impl<T: ?Sized> BoxExt for Box<T> {
    type T = T;

    fn drop_box(bx: Self) -> UninitBox {
        unsafe {
            let layout = Layout::for_value::<T>(&bx);
            let ptr = NonNull::new_unchecked(Box::into_raw(bx));

            ptr.as_ptr().drop_in_place();

            UninitBox {
                ptr: ptr.cast(),
                layout,
            }
        }
    }

    fn take_box(bx: Self) -> (UninitBox, Self::T)
    where
        Self::T: Sized,
    {
        unsafe {
            let ptr = NonNull::new_unchecked(Box::into_raw(bx));

            let value = ptr.as_ptr().read();

            (
                UninitBox {
                    ptr: ptr.cast(),
                    layout: Layout::new::<T>(),
                },
                value,
            )
        }
    }
}

/// An uninitialized piece of memory
pub struct UninitBox {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl UninitBox {
    /// The layout of the allocation
    #[inline]
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// create a new allocation that can fit the given type
    #[inline]
    pub fn new<T>() -> Self {
        Self::from_layout(Layout::new::<T>())
    }

    /// Create a new allocation that can fit the given layout
    #[inline]
    pub fn from_layout(layout: Layout) -> Self {
        if layout.size() == 0 {
            UninitBox {
                layout,
                ptr: unsafe { NonNull::new_unchecked(layout.align() as *mut u8) },
            }
        } else {
            let ptr = unsafe { std::alloc::alloc(layout) };

            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout)
            } else {
                unsafe {
                    UninitBox {
                        ptr: NonNull::new_unchecked(ptr),
                        layout,
                    }
                }
            }
        }
    }

    /// Initialize the box with the given value,
    ///
    /// # Panic
    ///
    /// if `std::alloc::Layout::new::<T>() != self.layout()` then
    /// this function will panic
    #[inline]
    pub fn init<T>(self, value: T) -> Box<T> {
        assert_eq!(
            self.layout,
            Layout::new::<T>(),
            "Layout of UninitBox is incompatible with `T`"
        );

        let bx = ManuallyDrop::new(self);

        let ptr = bx.ptr.cast::<T>().as_ptr();

        unsafe {
            ptr.write(value);

            Box::from_raw(ptr)
        }
    }

    /// Initialize the box with the given value,
    ///
    /// # Panic
    ///
    /// if `std::alloc::Layout::new::<T>() != self.layout()` then
    /// this function will panic
    #[inline]
    pub fn init_with<T, F: FnOnce() -> T>(self, value: F) -> Box<T> {
        assert_eq!(
            self.layout,
            Layout::new::<T>(),
            "Layout of UninitBox is incompatible with `T`"
        );

        let bx = ManuallyDrop::new(self);

        let ptr = bx.ptr.cast::<T>().as_ptr();

        unsafe {
            ptr.write(value());

            Box::from_raw(ptr)
        }
    }

    /// Get the pointer from the `UninitBox`
    ///
    /// This pointer is not valid to write to
    #[inline]
    pub fn as_ptr(&self) -> *const () {
        self.ptr.as_ptr() as *const ()
    }

    /// Get the pointer from the `UninitBox`
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut () {
        self.ptr.as_ptr() as *mut ()
    }
}

impl Drop for UninitBox {
    fn drop(&mut self) {
        unsafe { std::alloc::dealloc(self.ptr.as_ptr(), self.layout) }
    }
}
