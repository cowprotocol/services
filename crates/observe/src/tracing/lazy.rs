/// Helper struct to lazily evaluate an expression if a log is actually active.
/// Sometimes you need to compute a value for logs. This expression gets
/// evaluated eagerly in the `tracing` log macros. In order to only evaluate the
/// expression when the log is actually enabled wrap the expression in a closure
/// and wrap that in a [`Lazy`].
pub struct Lazy<T>(pub T);

impl<T, D> std::fmt::Debug for Lazy<T>
where
    T: Fn() -> D,
    D: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", (self.0)())
    }
}

impl<T, D> std::fmt::Display for Lazy<T>
where
    T: Fn() -> D,
    D: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", (self.0)())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lazy_eval() {
        let lazy = Lazy(|| "abc".to_string());

        let display = format!("{}", lazy);
        assert_eq!(display, "abc");

        let debug = format!("{:?}", lazy);
        assert_eq!(debug, "\"abc\"");
    }

    #[test]
    fn lazy_in_macro() {
        tracing::debug!(
            miep = ?Lazy(|| {
                panic!("this panic should not happen because we evaluate lazily");
                #[expect(unreachable_code)]
                "abc"
            })
        )
    }
}
