/// Store a value in the context using the variable name as key.
///
/// The value must implement `Serialize`. It is serialized with postcard.
/// The `ctx` parameter is the CellContext handle injected by the `#[cell]` macro.
///
/// # Examples
///
/// ```ignore
/// let data = vec![1, 2, 3];
/// store!(data);  // stores with key "data"
///
/// store!(my_key = some_value);  // stores with key "my_key"
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
/// The type must implement `DeserializeOwned`. Returns `Result<T>`.
/// The `ctx` parameter is the CellContext handle injected by the `#[cell]` macro.
///
/// # Examples
///
/// ```ignore
/// // Explicit type in macro
/// let data = load!(data as Vec<i32>)?;
///
/// // Type inferred from annotation
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
/// The `ctx` parameter is the CellContext handle injected by the `#[cell]` macro.
///
/// # Examples
///
/// ```ignore
/// remove!(temp_data);  // removes "temp_data" from context
/// ```
#[macro_export]
macro_rules! remove {
    ($ctx:expr, $name:ident) => {
        $ctx.remove(stringify!($name))
    };
}

/// Load and remove a value from the context in one operation.
///
/// The type must implement `DeserializeOwned`. Returns `Result<T>`.
/// This is useful when you want to transfer ownership of a value.
/// The `ctx` parameter is the CellContext handle injected by the `#[cell]` macro.
///
/// # Examples
///
/// ```ignore
/// // Explicit type in macro
/// let data = consume!(temp_data as Vec<i32>)?;
///
/// // Type inferred from annotation
/// let data: Vec<i32> = consume!(temp_data)?;
/// // temp_data is now removed from context
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
