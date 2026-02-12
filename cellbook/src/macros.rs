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

/// Store a value in the context with schema version metadata.
///
/// Differs from [`store!`] by requiring `StoreSchema` and writing
/// both full type path and `StoreSchema::VERSION` metadata.
/// This enables `loadv!`/`consumev!` to reject
/// schema-version mismatches before deserialization.
///
/// ```ignore
/// storev!(data);
/// storev!(my_key = some_value);
/// storev!(data, version = 2);
/// ```
#[macro_export]
macro_rules! storev {
    ($ctx:expr, $var:ident, version = $version:expr) => {
        $ctx.store_versioned_with(stringify!($var), &$var, $version)
    };
    ($ctx:expr, $name:ident = $value:expr, version = $version:expr) => {
        $ctx.store_versioned_with(stringify!($name), &$value, $version)
    };
    ($ctx:expr, $var:ident) => {
        $ctx.store_versioned(stringify!($var), &$var)
    };
    ($ctx:expr, $name:ident = $value:expr) => {
        $ctx.store_versioned(stringify!($name), &$value)
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

/// Load a value from the context with schema version checking.
///
/// Differs from [`load!`] by requiring `StoreSchema` and validating:
/// - full stored type path matches `T`
/// - stored schema version matches `T::VERSION`
///
/// ```ignore
/// let data = loadv!(data as MyType)?;
/// let data: MyType = loadv!(data)?;
/// let data: MyType = loadv!(data, version = 2)?;
/// ```
#[macro_export]
macro_rules! loadv {
    ($ctx:expr, $name:ident as $ty:ty, version = $version:expr) => {
        $ctx.load_versioned_with::<$ty>(stringify!($name), $version)
    };
    ($ctx:expr, $name:ident, version = $version:expr) => {
        $ctx.load_versioned_with(stringify!($name), $version)
    };
    ($ctx:expr, $name:ident as $ty:ty) => {
        $ctx.load_versioned::<$ty>(stringify!($name))
    };
    ($ctx:expr, $name:ident) => {
        $ctx.load_versioned(stringify!($name))
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

/// Load and remove a value with schema version checking.
///
/// Differs from [`consume!`] by requiring `StoreSchema` and validating
/// full type path + schema version before removal.
///
/// ```ignore
/// let data = consumev!(temp_data as MyType)?;
/// let data: MyType = consumev!(temp_data)?;
/// let data: MyType = consumev!(temp_data, version = 2)?;
/// ```
#[macro_export]
macro_rules! consumev {
    ($ctx:expr, $name:ident as $ty:ty, version = $version:expr) => {
        $ctx.consume_versioned_with::<$ty>(stringify!($name), $version)
    };
    ($ctx:expr, $name:ident, version = $version:expr) => {
        $ctx.consume_versioned_with(stringify!($name), $version)
    };
    ($ctx:expr, $name:ident as $ty:ty) => {
        $ctx.consume_versioned::<$ty>(stringify!($name))
    };
    ($ctx:expr, $name:ident) => {
        $ctx.consume_versioned(stringify!($name))
    };
}
