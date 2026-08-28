#[macro_export]
macro_rules! bfp {
    ($val:literal) => {
        ($val)
            .parse::<$crate::balancer_v2::swap::fixed_point::Bfp>()
            .unwrap()
    };
}
