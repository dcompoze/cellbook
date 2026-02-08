/// Store a value in the context.
///
/// Uses the variable name as the key.
/// Requires `Serialize`.
///
/// ```ignore
/// store!(data);
/// store!(my_key = some_value);
/// ```
#[macro_export]
macro_rules! store {
    ($ctx:expr, $var:ident) => {
        $ctx.store(stringify!($var), &$var)
    };
    ($ctx:expr, $name:ident = $value:expr) => {
        $ctx.store(stringify!($name), &$value)
    };
}

/// Load a value from the context.
///
/// Requires `DeserializeOwned`.
/// Returns `Result<T>`.
///
/// ```ignore
/// let data = load!(data as Vec<i32>)?;
/// let data: Vec<i32> = load!(data)?;
/// ```
#[macro_export]
macro_rules! load {
    ($ctx:expr, $name:ident as $ty:ty) => {
        $ctx.load::<$ty>(stringify!($name))
    };
    ($ctx:expr, $name:ident) => {
        $ctx.load(stringify!($name))
    };
}

/// Remove a value from the context.
///
/// Returns `true` if the key existed.
///
/// ```ignore
/// remove!(temp_data);
/// ```
#[macro_export]
macro_rules! remove {
    ($ctx:expr, $name:ident) => {
        $ctx.remove(stringify!($name))
    };
}

/// Load and remove a value in one operation.
///
/// Requires `DeserializeOwned`.
/// Returns `Result<T>`.
///
/// ```ignore
/// let data = consume!(temp_data as Vec<i32>)?;
/// let data: Vec<i32> = consume!(temp_data)?;
/// ```
#[macro_export]
macro_rules! consume {
    ($ctx:expr, $name:ident as $ty:ty) => {
        $ctx.consume::<$ty>(stringify!($name))
    };
    ($ctx:expr, $name:ident) => {
        $ctx.consume(stringify!($name))
    };
}
