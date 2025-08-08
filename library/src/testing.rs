use crate::{Element, Number};

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

#[macro_export]
macro_rules! formula {
    (plus($($op:ident($args:tt)),*)) => {
        Element::Plus(vec![$(formula!($op($args))),*])
    };
    (neg($op:ident($args:tt))) => {
        Element::Negate(Box::new(formula!($op($args))))
    };
    (pow($op:ident($args:tt), $op2:ident($args2:tt))) => {
        Element::Pow(Box::new(formula!($op($args))), Box::new(formula!($op2($args2))))
    };
    (multiply($($op:ident($args:tt)),*)) => {
        Element::Multiply(vec![$(formula!($op($args))),*])
    };
    (num($n:expr)) => {
        Element::Number(Number::from($n))
    };
}

macro_rules! formula_matches {
    ($a:expr, num($n:expr)) => {{
        let e: &Element = $a;
        e.is($n)
    }};
    ($a:expr, plus($($op:ident($args:tt)),*)) => {{
        let e: &Element = $a;
        match e {
            Element::Plus(elements) => {
                let mut elements_iter = elements.iter();
                (||{$({
                    let Some(this_element) = elements_iter.next() else {
                        return false;
                    };
                    if !formula_matches!(this_element, $op($args)) {
                        return false;
                    }
                };)*
                return true;
                })() && elements_iter.next().is_none()
            }
            _ => false,
        }
    }};
}

#[test]
fn test_formula_macro() {
    use crate::Element;

    let some_bool = 'idk: {
        if false {
            break 'idk false;
        }
        true
    };

    assert!(formula_matches!(&formula!(plus(num(123), num(123))), plus(num(123), num(123))));
    assert!(!formula_matches!(&formula!(plus(num(123), num(123))), plus(num(123))));
    assert!(formula_matches!(&formula!(num(123)), num(123)));

    let elem = formula!(pow(num(1), num(2)));
    let elem = formula!(num(123));
    let elem = formula!(neg(num(123)));
    let elem = formula!(plus(num(42), num(58)));
}
