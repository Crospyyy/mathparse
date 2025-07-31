#[macro_export]
macro_rules! fancy_assert_eq {
    ($a:expr, $b:expr) => {
        if $a != $b {
            panic!(
                "Assertion failed while comparing:\n\"{}\" with \"{}\":\n{:?} != {:?}",
                stringify!($a),
                stringify!($b),
                $a,
                $b
            );
        }
    };
}
