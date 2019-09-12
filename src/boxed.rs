use std::alloc::Layout;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;

pub trait BoxExt: Sized {
    type T: ?Sized;

    fn drop_box(bx: Self) -> UninitBox;

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

#[test]
fn uninit_box() {
    UninitBox::new::<u32>().init(0.0f32);
    UninitBox::array::<u32>(3).init((0.0f32, 0u32, 10i32));
    UninitBox::new::<()>().init(());
}

impl UninitBox {
    #[inline]
    pub fn layout(&self) -> Layout {
        self.layout
    }

    #[inline]
    pub fn new<T>() -> Self {
        Self::from_layout(Layout::new::<T>())
    }

    #[inline]
    pub fn array<T>(n: usize) -> Self {
        Self::from_layout(Layout::array::<T>(n).expect("Invalid array!"))
    }

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

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for UninitBox {
    fn drop(&mut self) {
        unsafe { std::alloc::dealloc(self.ptr.as_ptr(), self.layout) }
    }
}
