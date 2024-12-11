mod healthz;
mod info;
mod metrics;
mod quote;
mod reveal;
mod settle;
mod solve;

pub(super) use {
    healthz::healthz,
    info::info,
    metrics::metrics,
    quote::{quote, OrderError},
    reveal::reveal,
    settle::settle,
    solve::{solve, AuctionError},
};

pub(crate) fn deserialize_solution_id<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct SolutionIdVisitor;

    impl serde::de::Visitor<'_> for SolutionIdVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or integer representing a solution ID")
        }

        fn visit_u64<E>(self, value: u64) -> Result<u64, E>
        where
            E: serde::de::Error,
        {
            Ok(value)
        }

        fn visit_str<E>(self, value: &str) -> Result<u64, E>
        where
            E: serde::de::Error,
        {
            value.parse::<u64>().map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_any(SolutionIdVisitor)
}
