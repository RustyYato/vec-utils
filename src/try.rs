/// A stable version of [`core::ops::Try`].
pub trait Try {
    /// The type of this value when viewed as successful.
    type Ok;
    /// The type of this value when viewed as failed.
    type Error;

    /// A return of `Ok(t)` means that the
    /// execution should continue normally, and the result of `?` is the
    /// value `t`. A return of `Err(e)` means that execution should branch
    /// to the innermost enclosing `catch`, or return from the function.
    fn into_result(self) -> Result<Self::Ok, Self::Error>;

    /// Wrap an error value to construct the composite result. For example,
    /// `Result::Err(x)` and `Result::from_error(x)` are equivalent.
    fn from_error(v: Self::Error) -> Self;

    /// Wrap an OK value to construct the composite result. For example,
    /// `Result::Ok(x)` and `Result::from_ok(x)` are equivalent.
    fn from_ok(v: Self::Ok) -> Self;
}

impl<T, E> Try for Result<T, E> {
    type Ok = T;
    type Error = E;

    fn into_result(self) -> Result<<Self as Try>::Ok, <Self as Try>::Error> {
        self
    }
    fn from_error(v: <Self as Try>::Error) -> Self {
        Err(v)
    }
    fn from_ok(v: <Self as Try>::Ok) -> Self {
        Ok(v)
    }
}

/// The error type that results from applying the try operator (`?`) to a `None` value.
pub struct NoneError;

impl<T> Try for Option<T> {
    type Ok = T;
    type Error = NoneError;

    fn into_result(self) -> Result<<Self as Try>::Ok, <Self as Try>::Error> {
        self.ok_or(NoneError)
    }
    fn from_error(_v: <Self as Try>::Error) -> Self {
        None
    }
    fn from_ok(v: <Self as Try>::Ok) -> Self {
        Some(v)
    }
}

/// Unwraps a result or propagates its error.
#[macro_export]
macro_rules! r#try {
    ($expr:expr) => {
        match $crate::Try::into_result($expr) {
            Ok(val) => val,
            Err(err) => return $crate::Try::from_error(::core::convert::From::from(err)),
        }
    };
    ($expr:expr,) => {
        $crate::r#try!($expr)
    };
}
