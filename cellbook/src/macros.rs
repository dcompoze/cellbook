/// Store a value in the context using the variable name as key.
#[macro_export]
macro_rules! store {
    ($var:ident) => {
        $crate::context::store(stringify!($var), $var)
    };
    ($name:ident = $value:expr) => {
        $crate::context::store(stringify!($name), $value)
    };
}

/// Load a value from the context. Returns `Result<Arc<T>>`.
#[macro_export]
macro_rules! load {
    ($name:ident as $ty:ty) => {
        $crate::context::load::<$ty>(stringify!($name))
    };
}
