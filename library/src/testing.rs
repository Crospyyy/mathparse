mod some_future_work {
    use crate::Element;

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
    (plus($($op:ident $( ( $($args:tt)* ) )?),*)) => {
        Element::Plus(vec![$(formula!($op$(($($args)*))?)),*])
    };
    (neg($op:ident $( ( $($args:tt)* ) )?)) => {
        Element::Negate(Box::new(formula!($op$(($($args)*))?)))
    };
	(pow(
		$op:ident $( ( $($args:tt)* ) )?,
		$op2:ident $( ( $($args2:tt)* ) )?
	)) => {
		Element::Pow(
			// linke Seite
			Box::new(formula!($op$(($($args)*))?)),
			// rechte Seite
			Box::new(formula!($op2$(($($args2)*))?))
		)
	};
    (mul($( $op:ident  $( ( $($args:tt)* ) )? ),*)) => {
        Element::Multiply(vec![$(formula!($op$(($($args)*))?)),*])
    };
    (num($n:expr)) => {
        Element::Number(Number::from($n))
    };
    (var($s:expr)) => {
        Element::Variable($s.to_string())
    };
    ($e:expr) => {
        $e.clone()
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
    use crate::Element;
    use crate::Number;
    use astro_float::expr;
    use macros::{match_formula, return_tokens};

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

    #[test]
    fn test_proc_macros() {
        macro_rules! test_matching {
            ($l:ident($($formula:tt)*), $m:ident($($m_formula:tt)*), $result: pat) => {
                assert!(matches!(match_formula_proc!(formula!($l($($formula)*)), $m($($m_formula)*)), $result));
            };
        }
        assert!(match_formula!(formula!(num(123)), num(123)));
        let f = formula!(num(123));
        assert!(match_formula!(f, num(x)).is_some_and(|x| x == 123));
        assert!(!match_formula!(formula!(num(123)), num(122)));
        assert!(match_formula!(formula!(pow(num(123), num(456))), pow));

        // extrahiere basis und exponent aus pow
        let f = formula!(pow(num(2), num(3)));
        if let Some((base, exp)) = match_formula!(f, pow(num(x), num(x))) {
            assert_eq!(base, &Number::from(2));
            assert_eq!(exp, &Number::from(3));
        } else {
            panic!("erwartetes match für pow(num(2), num(3)) mit pow(num(x), num(y)))")
        }

        // pow ohne extraktion matcht
        assert!(match_formula!(formula!(pow(num(2), num(3))), pow));

        // neg extraction
        let f = formula!(neg(num(4)));
        assert!(match_formula!(f, neg(x)).is_some_and(|x| x == &Element::Number(Number::from(4))));

        // multiply ohne extraktion matcht
        let f = formula!(mul(num(5), num(6)));
        assert!(match_formula!(f, mul));

        // multiply extraction
        let f = formula!(mul(num(7), num(8)));
        if let Some((a, b)) = match_formula!(f, mul(num(x), num(x))) {
            assert_eq!(*a, Number::from(7));
            assert_eq!(*b, Number::from(8));
        } else {
            panic!("erwartetes match für multiply(num(7), num(8)) mit mul(num(x), num(y)))")
        }

        // plus ohne extraktion matcht
        assert!(match_formula!(formula!(plus(num(1), num(2), num(3))), plus));

        // plus extraction
        let f = formula!(plus(num(1), num(2), num(3)));
        if let Some((a, b, c)) = match_formula!(f, plus(num(x), num(x), num(x))) {
            assert_eq!(*a, Number::from(1));
            assert_eq!(*b, Number::from(2));
            assert_eq!(*c, Number::from(3));
        } else {
            panic!("erwartetes match für plus(num(1), num(2), num(3)) mit plus(num(x), num(y), num(z)))")
        }

        // any matcht immer
        let formulas = [
            formula!(num(9)),
            formula!(pow(num(1), num(2))),
            formula!(plus(num(1), num(2))),
            formula!(mul(num(3), num(4))),
            formula!(neg(num(5))),
        ];
        for f in formulas {
            println!("Checking formula: {}", f.get_debug_string());
            assert!(match_formula!(f, _));
        }

        let f = formula!(pow(num(1), num(2)));
        assert!(match_formula!(f, _));
        assert!(match_formula!(f, pow(_, _)));
        assert!(match_formula!(f, pow(num(1), _)));
        assert!(match_formula!(f, pow(_, num(2))));
        assert!(match_formula!(f, pow(num(1), num(2))));
        assert!(match_formula!(f, pow(num(1), num(2))));
        let f = formula!(pow(num(1), pow(num(2), num(3))));
        let (n1, n2, n3) = match_formula!(f, pow(num(x), pow(num(x), num(x)))).expect("This should not fail");
        {
            assert_eq!(n1, &Number::from(1));
            assert_eq!(n2, &Number::from(2));
            assert_eq!(n3, &Number::from(3));
        }

        // fehlgeschlagene patterns
        assert!(!match_formula!(formula!(num(10)), num(11)));
        assert!(!match_formula!(formula!(pow(num(1), num(2))), pow(num(1), num(3))));
        assert!(match_formula!(formula!(var("hallo")), var("hallo")));
        assert!(!match_formula!(formula!(var("hallo")), var("tschüss")));
        let f = formula!(var("hallo"));
        let m_str = match_formula!(f, var(x));
        assert!(matches!(m_str, Some(s) if s == "hallo"));
        return_tokens!(outer(inner)..);
    }
}
