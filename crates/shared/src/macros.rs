#[macro_export]
macro_rules! externalprices {
    (native_token: $nt:expr_2021 $(, $($t:tt)*)?) => {
        $crate::external_prices::ExternalPrices::try_new(
            $nt,
            ::maplit::hashmap!($($($t)*)*),
        )
        .unwrap()
    };
}
