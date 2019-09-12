#![feature(try_trait, alloc_layout_extra)]

/// This allows running destructors, even if other destructors have panicked
macro_rules! defer {
    ($($do_work:tt)*) => {
        let _guard = $crate::OnDrop(Some(|| { $($do_work)* }));
    }
}

/// A macro to give syntactic sugar for `general_zip::try_zip_with`
///
/// This allows combining multiple vectors into a one with short-circuiting
/// on the failure case
///
/// # Usage
///
/// ```rust
/// # use vec_utils::try_zip_with;
/// # let vec_1: Vec<()> = Vec::new();
/// # let vec_2: Vec<()> = Vec::new();
/// # let vec_n: Vec<()> = Vec::new();
/// # let value = Ok::<(), ()>(());
/// try_zip_with!((vec_1, vec_2, vec_n), |x1, x2, xn| value);
/// ```
/// `value` can be any expression using `x1`, `x2`, `xn` or any other values from the environment
///
/// Note that `|x1, x2, xn| value` is not a closure, but some syntax that looks like a closure. In particular you
/// cannot use general patterns for the parameters, only identifiers are allowed. Second, you can't pass in a closure
/// like so,
///
/// ```rust compile_fail
/// # use vec_utils::try_zip_with;
/// # let vec_1: Vec<()> = Vec::new();
/// # let vec_2: Vec<()> = Vec::new();
/// # let vec_n: Vec<()> = Vec::new();
/// # let value = Ok::<(), ()>(());
/// try_zip_with!((vec_1, vec_2, vec_n), closure)
/// ```
///
/// But it will work just like a move closure in all other cases.
///
/// The first call will desugar to
///
/// ```rust
/// # let vec_1: Vec<()> = Vec::new();
/// # let vec_2: Vec<()> = Vec::new();
/// # let vec_n: Vec<()> = Vec::new();
/// # let value = Ok::<(), ()>(());
/// vec_utils::general_zip::try_zip_with((vec_1, (vec_2, (vec_n,))), move |(x1, (x2, xn))| value);
/// ```
#[macro_export]
macro_rules! try_zip_with {
    (($($vec:expr),+ $(,)?), |$($i:ident),+ $(,)?| $($work:tt)*) => {{
        $(let $i = $vec;)*

        $crate::general_zip::try_zip_with(
            $crate::list!(WRAP $($i),*),
            |$crate::list!(PLACE $($i),*)| $($work)*
        )
    }};
}

/// A wrapper around `try_zip_with` for infallible mapping
#[macro_export]
macro_rules! zip_with {
    (($($vec:expr),+ $(,)?), |$($i:ident),+ $(,)?| $($work:tt)*) => {{
        $crate::general_zip::unwrap($crate::try_zip_with!(
            ($($vec),+), |$($i),+|
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

struct OnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        self.0.take().unwrap()()
    }
}

pub mod test;

mod boxed;
mod vec;

pub use self::boxed::*;
pub use self::vec::*;
