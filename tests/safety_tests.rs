
// These test check if any of the functions leak or double drop elements

use vec_utils::*;

mod drop_counter {
    use std::sync::RwLock;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::fmt::Debug;
    use std::any::{Any, TypeId};

    pub struct DropCounter(RwLock<Vec<AtomicBool>>);

    pub struct OnDrop<'a, T: Debug + Any>(&'a RwLock<Vec<AtomicBool>>, usize, TypeId, T);

    impl DropCounter {
        pub fn new() -> Self {
            DropCounter(RwLock::default())
        }

        fn init(&self) -> usize {
            let mut lock = self.0.write().unwrap();
            let len = lock.len();
            lock.push(AtomicBool::new(false));
            len
        }

        pub fn create<T: Debug + Any>(&self, value: T) -> OnDrop<'_, T> {
            let len = self.init();
            OnDrop(&self.0, len, TypeId::of::<T>(), value)
        }
    }

    impl<T: Debug + Any> OnDrop<'_, T> {
        pub fn get(&self) -> &T {
            &self.3
        }
    }

    impl<T: Debug + Any> Drop for OnDrop<'_, T> {
        fn drop(&mut self) {
            if !std::thread::panicking() {
                assert_eq!(TypeId::of::<T>(), self.2, "Incorrect type punning detected!");
            }

            let count = self.0.read().unwrap();
            
            let was_droppped = count[self.1].fetch_or(true, Ordering::Relaxed);

            drop(count);

            if was_droppped {
                if std::thread::panicking() {
                    println!("Double dropped {:?}", self.3);
                } else {
                    panic!("Double dropped {:?}", self.3);
                }
            }
        }
    }

    impl Drop for DropCounter {
        fn drop(&mut self) {
            let count = self.0.get_mut().unwrap();

            let leaked = count.iter_mut().enumerate().fold(Vec::new(), |mut leaked, (i, was_droppped)| {
                if !*was_droppped.get_mut() {
                    leaked.push(i);
                }

                leaked
            });

            if !leaked.is_empty() {
                if std::thread::panicking() {
                    println!("Detected leak: {:?}", leaked)
                } else {
                    panic!("Detected leak: {:?}", leaked)
                }
            }
        }
    }
    
    #[test]
    fn simple() {
        let _dr = DropCounter::new();
    }

    #[test]
    fn create() {
        let dr = DropCounter::new();
        
        dr.create(());
    }

    #[test]
    fn create_many() {
        let dr = DropCounter::new();
        
        let _ = (0..100).map(|_| dr.create(())).collect::<Vec<_>>();
    }

    #[test]
    #[should_panic]
    fn leak() {
        let dr = DropCounter::new();
        
        std::mem::forget(dr.create("leak"));
    }

    #[test]
    #[should_panic]
    fn double_drop() {
        let dr = DropCounter::new();
        
        unsafe {
            std::ptr::drop_in_place(&mut dr.create("drop twice"));
        }
    }
}

use drop_counter::DropCounter;

mod boxed {
    use super::*;

    #[test]
    fn drop() {
        let dr = DropCounter::new();
        
        let bx = Box::new(dr.create("drop once"));

        let _uninit = Box::drop_box(bx);
    }

    #[test]
    fn init() {
        let dr = DropCounter::new();
        
        let bx = Box::new(dr.create("drop once"));

        let uninit = Box::drop_box(bx);

        uninit.init(dr.create("init"));
    }

    #[test]
    fn take() {
        let dr = DropCounter::new();
        
        let bx = Box::new(dr.create("drop once"));

        let (_uninit, _value) = Box::take_box(bx);
    }

    #[test]
    fn take_re_init() {
        let dr = DropCounter::new();
        
        let bx = Box::new(dr.create("drop once"));

        let (uninit, value) = Box::take_box(bx);

        uninit.init(value);
    }
}

mod vec {
    #![allow(unused_assignments)]
    use super::*;

    #[test]
    fn map() {
        let dr = DropCounter::new();
        
        let vec = (0..10).map(|x| dr.create(x)).collect::<Vec<_>>();

        vec.map(|x| dr.create(*x.get()));
    }

    #[test]
    fn try_map() {
        let dr = DropCounter::new();
        
        let vec = (0..10).map(|x| dr.create(x)).collect::<Vec<_>>();

        let mut counter = 0;

        let err = vec.try_map(|x| {
            counter += 1;

            if counter == 4 {
                None
            } else {
                Some(dr.create(*x.get() as f32))
            }
        }).is_err();

        assert!(err);
    }

    #[test]
    fn zip_with_same() {
        let dr = DropCounter::new();
        
        let mut a = (0..10).map(|x| dr.create(x)).collect::<Vec<_>>();
        let b = (20..30).map(|x| dr.create(x)).collect::<Vec<_>>();

        let mut flip = false;

        a = a.zip_with(b, |x, y| {
            flip = !flip;

            if flip {
                x
            } else {
                y
            }
        });
    }
    
    #[test]
    fn zip_with_diff() {
        let dr = DropCounter::new();
        
        let a = (0..10).map(|x| dr.create(x)).collect::<Vec<_>>();
        let mut b = (20..30).map(|x| dr.create(x)).collect::<Vec<_>>();

        b.reserve(10);

        let mut flip = false;

        b = a.zip_with(b, |x, y| {
            flip = !flip;

            if flip {
                x
            } else {
                y
            }
        });
    }
    
    #[test]
    fn try_zip_with() {
        let dr = DropCounter::new();
        
        let a = (0..10).map(|x| dr.create(x)).collect::<Vec<_>>();
        let mut b = (20..30).map(|x| dr.create(x)).collect::<Vec<_>>();

        b.reserve(10);

        let mut flip = false;
        let mut counter = 0;

        let err = a.try_zip_with(b, |x, y| {
            flip = !flip;
            counter += 1;

            if counter == 5 {
                None
            } else {
                Some(dr.create((*x.get() + *y.get()) as f32))
            }
        }).is_err();

        assert!(err);
    }
}
