mod some_future_work {
    macro_rules! write_if_true {
        (true, $($tts:tt),+) => {
            $($tts),+
        };
        (false, $($tts:tt),+)=>{}
    }

    macro_rules! any {
        () => {
            false
        };
        (true) => {true};
        (false) => {false};
        (true, $($l:tt),+) => {
            true
        };
        (false, $($l:tt),+) => {
            any!($($l),*)
        };
    }

    // #[test]
    // fn idk() {
    //     let x = any!(false, false);
    // }

    macro_rules! input_contains_var {
        ($($tts:tt),+) => {
            input_contains_var!()
        };
    }
}

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

#[macro_export]
macro_rules! formula_matches {
    ($a:expr, any(_)) => {true};
    ($a:expr, num(_)) => {matches!($a, Element::Number(_))};
    ($a:expr, plus(_)) => {matches!($a, Element::Plus(_))};
    ($a:expr, mul(_)) => {matches!($a, Element::Multiply(_))};
    ($a:expr, pow(_)) => {matches!($a, Element::Pow(..))};
    ($a:expr, num(x)) => {
        match $a {
            Element::Number(n) => Some(n),
            _ => None,
        }
    };
    ($a:expr, num($n:expr)) => {$a.is($n)};
    ($a:expr, plus($($op:ident($args:tt)),*)) => {
        match $a {
            Element::Plus(elements) => {
                let mut elements_iter = elements.iter();
                $(elements_iter.next().is_some_and(|e| formula_matches!(e, $op($args)))) && * && elements_iter.next().is_none()
            }
            _ => false,
        }
    };
    ($a:expr, mul($($op:ident($args:tt)),*)) => {
        match $a {
            Element::Multiply(elements) => {
                let mut elements_iter = elements.iter();
                $(elements_iter.next().is_some_and(|e| formula_matches!(e, $op($args))))&& * && elements_iter.next().is_none()
            }
            _ => false,
        }
    };
    ($a:expr, neg($op:ident($args:tt))) => {
        match $a {
            Element::Negate(e) => {
                formula_matches!(e, $op($args))
            }
            _ => false,
        }
    };

    ($a:expr, pow($op1:ident($args1:tt), $op2:ident($args2:tt))) => {
        match $a {
            Element::Pow(b, e) => {
                formula_matches!(&*b, $op1($args1)) && formula_matches!(&*e, $op2($args2))
            }
            _ => false,
        }
    };
}

#[cfg(test)]
mod test {
    use crate::Number;

    macro_rules! verify_formula_matches {
        ($($tts:tt)+) => {assert!(formula_matches!(formula!($($tts)+),$($tts)+))};
    }

    #[test]
    fn test_formula_macro() {
        use crate::Element;

        formula_matches!(formula!(num(123)), num(_));
        formula_matches!(formula!(pow(num(123), num(123))), pow(_));
        formula_matches!(formula!(pow(num(123), num(123))), pow(num(_), any(_)));
        formula_matches!(formula!(pow(num(123), num(123))), pow(any(_), num(_)));
        formula_matches!(formula!(pow(num(123), num(123))), pow(num(_), num(_)));
        formula_matches!(formula!(pow(num(123), num(123))), pow(num(_), num(_)));
        formula_matches!(formula!(num(123)), num(_));
        verify_formula_matches!(num(123));
        verify_formula_matches!(neg(num(123)));
        verify_formula_matches!(num(123));
        verify_formula_matches!(neg(num(123)));
        verify_formula_matches!(pow(num(123), num(123)));
        verify_formula_matches!(plus(num(123), num(123)));
    }
}
