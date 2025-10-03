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

#[cfg(test)]
mod test {
	use crate::Element;
	use crate::Number;
	use macros::{formula_matches, return_tokens};

	#[test]
	fn test_proc_macros() {
		assert!(formula_matches!(formula!(num(123)), num(123)));
		let f = formula!(num(123));
		assert!(formula_matches!(f, num(x)).is_some_and(|x| x == 123));
		assert!(formula_matches!(formula!(num(123)), num(123)));
		assert!(formula_matches!(formula!(num(123)), num(122 + 1)));
		assert!(!formula_matches!(formula!(num(123)), num(122)));
		assert!(formula_matches!(formula!(pow(num(123), num(456))), pow));

		// extrahiere basis und exponent aus pow
		let f = formula!(pow(num(2), num(3)));
		assert!(formula_matches!(f, { &f }));
		if let Some((base, exp)) = formula_matches!(f, pow(num(x), num(x))) {
			assert_eq!(base, &Number::from(2));
			assert_eq!(exp, &Number::from(3));
		} else {
			panic!("erwartetes match für pow(num(2), num(3)) mit pow(num(x), num(y)))")
		}

		// pow ohne extraktion matcht
		assert!(formula_matches!(formula!(pow(num(2), num(3))), pow));

		// neg extraction
		let f = formula!(neg(num(4)));
		assert!(formula_matches!(f, neg(x)).is_some_and(|x| x == &Element::Number(Number::from(4))));

		// multiply ohne extraktion matcht
		let f = formula!(mul(num(5), num(6)));
		assert!(formula_matches!(f, mul));

		// multiply extraction
		let f = formula!(mul(num(7), num(8)));
		if let Some((a, b)) = formula_matches!(f, mul(num(x), num(x))) {
			assert_eq!(a, &Number::from(7));
			assert_eq!(b, &Number::from(8));
		} else {
			panic!("erwartetes match für multiply(num(7), num(8)) mit mul(num(x), num(y)))")
		}

		// plus ohne extraktion matcht
		let r = formula_matches!(formula!(plus(num(1), num(2), num(3))), plus);
		assert!(r);

		// plus extraction
		let f = formula!(plus(num(1), num(2), num(3)));
		if let Some((a, b, c)) = formula_matches!(f, plus(num(x), num(x), num(x))) {
			assert_eq!(a, &Number::from(1));
			assert_eq!(b, &Number::from(2));
			assert_eq!(c, &Number::from(3));
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
			assert!(formula_matches!(f, _));
		}

		let f = formula!(pow(num(1), num(2)));
		assert!(formula_matches!(f, _));
		assert!(formula_matches!(f, pow(_, _)));
		let m = formula_matches!(f, pow(num(1), _));
		assert!(m);
		assert!(formula_matches!(f, pow(_, num(2))));
		assert!(formula_matches!(f, pow(num(1), num(2))));
		assert!(formula_matches!(f, pow(num(1), num(2))));
		let f = formula!(pow(num(1), pow(num(2), num(3))));
		let (n1, n2, n3) =
			formula_matches!(f, pow(num(x), pow(num(x), num(x)))).expect("This should not fail");
		{
			assert_eq!(n1, &Number::from(1));
			assert_eq!(n2, &Number::from(2));
			assert_eq!(n3, &Number::from(3));
		}

		// fehlgeschlagene patterns
		assert!(!formula_matches!(formula!(num(10)), num(11)));
		assert!(!formula_matches!(formula!(pow(num(1), num(2))), pow(num(1), num(3))));
		assert!(formula_matches!(formula!(var("hallo")), var("hallo")));
		assert!(!formula_matches!(formula!(var("hallo")), var("tschüss")));
		let f = formula!(var("hallo"));
		let m_str = formula_matches!(f, var(x));
		assert!(matches!(m_str, Some(s) if s == "hallo"));
		return_tokens!(outer(inner)..);
	}
}
