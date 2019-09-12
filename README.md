# vec-utils

This is an experimental crate that adds some helpful functionality to `Vec<T>`, like `map` and `zip_with`. These functions allow you to transform a vec and try and reuse the allocation if possible!

It also adds some functionality to reuse the allocation of a `Box<T>`, using the `UninitBox` api.

This crate also exports some macros that are more flexible than the given functions, and in most cases will compile down to the same assembly as `Vec::map` and `Vec::zip_with`