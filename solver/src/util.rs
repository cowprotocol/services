// This macro allows to build literal iterations without allocating
// a container such as a vector:
// chain![1, 2, 3]
#[macro_export]
macro_rules! chain {
    ( $x:expr ) => {
        {
            use std::iter::once;
            once($x)
        }
    };
    ( $first_x:expr, $( $further_x:expr ),+ ) => {
        {
            use std::iter::once;
            let temp_iter = once($first_x);
            $(
                let temp_iter = temp_iter.chain(once($further_x));
            )+
            temp_iter
        }
    };
}
