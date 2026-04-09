pub mod autopilot;
pub mod balance_overrides;
pub mod banned_users;
pub mod database;
pub(crate) mod deserialize_env;
pub mod fee_factor;
pub mod gas_price_estimation;
pub mod http_client;
pub mod native_price;
pub mod native_price_estimators;
pub mod order_quoting;
pub mod orderbook;
pub mod price_estimation;
pub mod rate_limit;
pub mod shared;
pub mod simulator;

#[cfg(any(test, feature = "test-util"))]
pub mod test_util {
    /// Provides standard test defaults while keeping separate from the regular
    /// `Default` trait.
    pub trait TestDefault {
        /// Returns a test specific default.
        ///
        /// For example, when providing a default for a database connection for
        /// tests, this can return a `localhost` connection.
        fn test_default() -> Self;
    }

    // No blanket implementation due to lack of specialization features:
    // https://github.com/rust-lang/rfcs/blob/master/text/1210-impl-specialization.md
}
