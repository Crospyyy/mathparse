use crate::{Element, Number};
use astro_float::{BigFloat, Error};
use num_traits::{Signed, ToPrimitive};
use std::cmp::PartialEq;
use strum::{EnumCount, IntoEnumIterator};
use strum_macros::{EnumCount, EnumIter};

#[derive(EnumIter, EnumCount, Copy, Clone, PartialEq, Debug)]
pub enum Optimization {
    CombineExponents,
    OneOrZeroToAnyPower,
    MultiplyByOne,
    PlusZero,
    DoubleNegation,
    DivideBySame,
    MultiplyByZero,
    FlattenPlus,
    FlattenMultiply,
    PowerOfZero,
    FlattenPower,
}

impl PartialEq<i32> for Number {
    fn eq(&self, other: &i32) -> bool {
        match self {
            Number::Rational(r) => r.is_integer() && r.to_i32() == Some(*other),
            Number::Float(f) => f.is_int() && f == &BigFloat::from(*other),
        }
    }
}

impl PartialEq<i32> for &Number {
    fn eq(&self, other: &i32) -> bool {
        match self {
            Number::Rational(r) => r.is_integer() && r.to_i32() == Some(*other),
            Number::Float(f) => f.is_int() && f == &BigFloat::from(*other),
        }
    }
}

impl PartialEq<i32> for &Element {
    fn eq(&self, other: &i32) -> bool {
        self.get_number_inner().is_some_and(|n| n == other)
    }
}

impl Element {
    pub(crate) fn is(&self, other: i32) -> bool {
        self.get_number_inner().is_some_and(|n| *n == other)
    }

    pub(crate) fn is_neg(&self, other: i32) -> bool {
        self.get_negate_inner().is_some_and(|e| e.is(other))
    }
}

impl Number {
    pub(crate) fn is_negative(&self) -> bool {
        match self {
            Number::Rational(r) => r.is_negative(),
            Number::Float(f) => f.is_negative(),
        }
    }
}

impl Element {
    pub fn optimize_all(&mut self) -> Vec<Optimization> {
        let mut all_optimizations = Vec::new();
        loop {
            let mut any_optimization = false;
            for optimization in Optimization::iter() {
                dbg!(optimization);
                if self.optimize(optimization) {
                    all_optimizations.push(optimization);
                    any_optimization = true;
                }
            }
            if !any_optimization {
                break;
            }
        }
        all_optimizations
    }

    fn optimize(&mut self, optimization: Optimization) -> bool {
        if self.anything_unparsed() {
            return false;
        }

        let mut found_optimization =
            self.run_on_children(&mut |element| Self::optimize(element, optimization));
        match optimization {
            Optimization::CombineExponents => {
                if let Element::Pow(base, exponent) = self {
                    if let Element::Pow(base_inner, exponent_inner) = base.as_mut() {
                        *self = Element::Pow(
                            Box::new(*base_inner.clone()),
                            Box::new(Element::Multiply(vec![*exponent_inner.clone(), *exponent.clone()])),
                        );
                        found_optimization = true;
                    }
                }
            },
            Optimization::OneOrZeroToAnyPower => {
                if let Element::Pow(base, exp) = self {
                    if base.is(1) {
                        *self = Element::Number(Number::from(1));
                        found_optimization = true;
                        return found_optimization;
                    } else if base.is(0) {
                        if exp.get_number_inner().is_some_and(|n| n.is_negative()) {
                            *self = Element::Number(Number::nan(Some(Error::DivisionByZero)));
                        } else {
                            *self = Element::Number(Number::from(0));
                        }
                        found_optimization = true;
                        return found_optimization;
                    }
                }
            },
            Optimization::MultiplyByOne => {
                if let Element::Multiply(elements) = self {
                    let len_before = elements.len();
                    elements.retain(|e| !e.is(1));
                    found_optimization |= len_before != elements.len();
                    if elements.is_empty() {
                        *self = Element::Number(Number::from(1));
                    } else if elements.len() == 1 {
                        *self = elements.remove(0);
                    }
                }
            },
            Optimization::PlusZero => {
                if let Element::Plus(elements) = self {
                    let len_before = elements.len();
                    elements.retain(|e| !e.is(0));
                    found_optimization |= len_before != elements.len();
                    if elements.is_empty() {
                        *self = Element::Number(Number::from(0));
                    } else if elements.len() == 1 {
                        *self = elements.remove(0);
                    }
                }
            },
            Optimization::DoubleNegation => {
                if let Element::Negate(element) = self {
                    if let Element::Negate(inner_element) = &**element {
                        *self = *inner_element.clone();
                        found_optimization = true;
                    }
                }
            },
            Optimization::DivideBySame => {
                fn b_is_inverse_of_a(a: &Element, b: &Element) -> bool {
                    b.get_pow_inner()
                        .is_some_and(|(b_base, b_exponent)| b_exponent.is_neg(1) && *a == *b_base)
                }
                if let Element::Multiply(elements) = self {
                    if Self::contains_division_by_zero(elements) {
                        *self = Element::Number(Number::nan(Some(Error::DivisionByZero)));
                        return found_optimization;
                    }
                    let mut to_remove = vec![false; elements.len()];
                    let mut found = false;
                    for i in 0..elements.len() {
                        if to_remove[i] {
                            continue;
                        }
                        for j in (i + 1)..elements.len() {
                            if to_remove[j] {
                                continue;
                            }

                            let (a, b) = (&elements[i], &elements[j]);
                            if b_is_inverse_of_a(a, b) || b_is_inverse_of_a(b, a) {
                                to_remove[i] = true;
                                to_remove[j] = true;
                                found = true;
                                break;
                            }
                        }
                    }
                    if found {
                        found_optimization = true;
                        let mut new_elements: Vec<_> = elements
                            .iter()
                            .zip(to_remove.iter())
                            .filter(|(_, remove)| !**remove)
                            .map(|(el, _)| el.clone())
                            .collect();
                        if new_elements.is_empty() {
                            *self = Element::Number(Number::from(1));
                        } else if new_elements.len() == 1 {
                            *self = new_elements.remove(0);
                        } else {
                            *elements = new_elements;
                        }
                    }
                }
            },
            Optimization::MultiplyByZero => {
                if let Element::Multiply(elements) = self {
                    if Self::contains_division_by_zero(elements) {
                        *self = Element::Number(Number::nan(Some(Error::DivisionByZero)));
                        return found_optimization;
                    }
                    if elements.iter().any(|e| e.is(0)) {
                        *self = Element::Number(Number::from(0));
                        found_optimization = true;
                    }
                }
            },
            Optimization::FlattenPlus => {
                if let Element::Plus(elements) = self {
                    let mut could_flatten = false;
                    let new_elements: Vec<_> = elements
                        .iter()
                        .flat_map(|e| {
                            if let Element::Plus(inner_elements) = e {
                                found_optimization = true;
                                could_flatten = true;
                                inner_elements.clone().into_iter()
                            } else {
                                vec![e.clone()].into_iter()
                            }
                        })
                        .collect();
                    if could_flatten {
                        *elements = new_elements;
                    }
                }
            },
            Optimization::FlattenMultiply => {
                if let Element::Multiply(elements) = self {
                    if !elements.iter().any(|e| matches!(e, Element::Multiply(_))) {
                        return found_optimization;
                    }
                    let new_elements: Vec<_> = elements
                        .iter()
                        .flat_map(|e| {
                            if let Element::Multiply(inner_elements) = e {
                                inner_elements.clone().into_iter()
                            } else {
                                vec![e.clone()].into_iter()
                            }
                        })
                        .collect();

                    found_optimization = true;
                    *elements = new_elements;
                }
            },
            Optimization::PowerOfZero => {
                if self.get_pow_inner().is_some_and(|(_b, e)| e.is(0)) {
                    *self = Element::Number(Number::from(1));
                    found_optimization = true;
                }
            },
            Optimization::FlattenPower => {
                if let Some((base, exp)) = self.get_pow_inner_mut() {
                    if let Some(inner) = base.get_mul_inner() {
                        let new_elements = inner
                            .iter()
                            .map(|b| Element::Pow(Box::new(b.clone()), Box::new(exp.clone())))
                            .collect();
                        *self = Element::Multiply(new_elements);
                        found_optimization = true;
                    }
                }
            },
        }
        found_optimization
    }

    fn contains_division_by_zero(elements: &mut Vec<Element>) -> bool {
        elements.iter().any(|e| {
            e.get_pow_inner()
                .is_some_and(|(b, e)| b == 0 && e.get_number_inner().is_some_and(|n| n.is_negative()))
        })
    }

    fn run_on_children(&mut self, operation: &mut impl Fn(&mut Element) -> bool) -> bool {
        match self {
            Element::Plus(elements)
            | Element::Multiply(elements)
            | Element::Function { arguments: elements, .. }
            | Element::FunctionWithExpression { arguments: elements, .. } => {
                elements.iter_mut().map(|arg| operation(arg)).reduce(|a, b| a || b).unwrap_or(false)
            },
            Element::Pow(base, exponent) => operation(base) || operation(exponent),
            Element::Negate(element) => operation(element),
            Element::Variable(_)
            | Element::Brackets(_)
            | Element::String(_)
            | Element::VariableOrFunction(_)
            | Element::Number(_)
            | Element::NumberWithExpression(_) => false,
        }
    }
}

macro_rules! implement_internal {
    ($name:ident, $name_mut:ident, $return_type:ty, $enum_name:ident, $inner_name:ident) => {
        fn $name(&self) -> Option<&$return_type> {
            match self {
                Element::$enum_name($inner_name) => Some($inner_name),
                _ => None,
            }
        }

        fn $name_mut(&mut self) -> Option<&mut $return_type> {
            match self {
                Element::$enum_name($inner_name) => Some($inner_name),
                _ => None,
            }
        }
    };
    ($name:ident, $name_mut:ident, $return_type:ty, $enum_name:ident, $inner_name:ident, $inner_name2:ident) => {
        fn $name(&self) -> Option<(&$return_type, &$return_type)> {
            match self {
                Element::$enum_name($inner_name, $inner_name2) => Some((&**$inner_name, &**$inner_name2)),
                _ => None,
            }
        }

        fn $name_mut(&mut self) -> Option<(&mut $return_type, &mut $return_type)> {
            match self {
                Element::$enum_name($inner_name, $inner_name2) => Some(($inner_name, $inner_name2)),
                _ => None,
            }
        }
    };
}

impl Element {
    implement_internal!(get_number_inner, get_number_inner_mut, Number, Number, num);
    implement_internal!(get_pow_inner, get_pow_inner_mut, Element, Pow, base, exponent);
    implement_internal!(get_negate_inner, get_negate_inner_mut, Element, Negate, element);
    implement_internal!(get_variable_inner, get_variable_inner_mut, String, Variable, name);
    fn get_mul_inner(&self) -> Option<&Vec<Self>> {
        if let Element::Multiply(inner) = self { Some(inner) } else { None }
    }
    fn get_mul_inner_mut(&mut self) -> Option<&mut Vec<Self>> {
        if let Element::Multiply(inner) = self { Some(inner) } else { None }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_short::{inv, mul, neg, num, pow, var};

    fn check_input_and_output_match(input: &str, expected_output: &str, optimization: Optimization) {
        check_input_output_formula_match(input, Element::parse(expected_output).unwrap(), optimization);
    }

    fn check_input_output_formula_match(input: &str, expected_out: Element, optimization: Optimization) {
        let mut formula = Element::parse(input).unwrap();
        let success = formula.optimize(optimization);

        assert!(success);
        assert_eq!(formula, expected_out);
    }
    fn check_input_output_formula_both_match(
        input: Element, expected_out: Element, optimization: Optimization,
    ) {
        let mut formula = input;
        let success = formula.optimize(optimization);

        assert!(success);
        assert_eq!(formula, expected_out);
    }

    fn check_input_output_formula_both_match_also_complete(
        input: Element, expected_out: Element, optimization: Optimization,
    ) {
        {
            println!("Optimizing with the step alone");
            let mut formula = input.clone();
            let success = formula.optimize(optimization);

            assert!(success);
            assert_eq!(formula, expected_out);
        }
        {
            println!("Optimizing all");
            let mut formula = input.clone();
            let success = formula.optimize_all();

            assert!(success.contains(&optimization));
            assert_eq!(formula, expected_out);
        }
    }

    #[test]
    fn test_combine_exponents() {
        check_input_and_output_match("(a^2)^3", "a^(2*3)", Optimization::CombineExponents);
    }

    #[test]
    fn test_one_or_zero_to_any_power() {
        check_input_and_output_match("1^-1", "1", Optimization::OneOrZeroToAnyPower);
        check_input_and_output_match("1^x", "1", Optimization::OneOrZeroToAnyPower);
        check_input_and_output_match("0^-1", "0", Optimization::OneOrZeroToAnyPower);
        check_input_and_output_match("0^x", "0", Optimization::OneOrZeroToAnyPower);
    }

    #[test]
    fn test_multiply_by_one() {
        check_input_output_formula_match("a*1", var("a"), Optimization::MultiplyByOne);
    }

    #[test]
    fn test_plus_zero() {
        check_input_output_formula_match("a+0", var("a"), Optimization::PlusZero);
    }

    #[test]
    fn test_double_negation() {
        check_input_output_formula_match("--a", var("a"), Optimization::DoubleNegation);
    }

    #[test]
    fn test_divide_by_same() {
        check_input_output_formula_both_match(
            mul([var("a"), inv(var("a"))]),
            num(1),
            Optimization::DivideBySame,
        );
        check_input_output_formula_both_match(
            mul([var("a"), num(1), inv(var("a"))]),
            num(1),
            Optimization::DivideBySame,
        );
    }

    #[test]
    fn test_multiply_by_zero() {
        check_input_and_output_match("a*0", "0", Optimization::MultiplyByZero);
    }

    #[test]
    fn test_flatten_plus() {
        check_input_and_output_match("a+(b+c)", "a+b+c", Optimization::FlattenPlus);
    }

    #[test]
    fn test_flatten_multiply() {
        // check_input_and_output_match("a*(b*c)", "a*b*c", Optimization::FlattenMultiply);
        // check_input_output_formula_both_match(
        //     mul([var("a"), mul([var("b"), var("c")])]),
        //     mul([var("a"), var("b"), var("c")]),
        //     Optimization::FlattenMultiply,
        // );
        check_input_output_formula_both_match_also_complete(
            mul([num(33), mul([pow(num(2), neg(num(1))), pow(num(34), neg(num(1)))])]),
            mul([num(33), pow(num(2), neg(num(1))), pow(num(34), neg(num(1)))]),
            Optimization::FlattenMultiply,
        );
    }

    #[test]
    fn test_power_of_zero() {
        check_input_output_formula_match("a^0", num(1), Optimization::PowerOfZero);
        check_input_output_formula_match("0^0", num(1), Optimization::PowerOfZero);
        check_input_output_formula_match("1^0", num(1), Optimization::PowerOfZero);
        check_input_output_formula_match("2^0", num(1), Optimization::PowerOfZero);
    }

    #[test]
    fn test_optimize_all() {
        // Testfälle: (Eingabeformel, Erwartete Ausgabe, Erwartete Optimierungen)
        let test_cases = vec![
            (
                "-(-a)*1 + (0+b) + c*(d*0)",
                "a+b",
                vec![
                    Optimization::DoubleNegation,
                    Optimization::MultiplyByOne,
                    Optimization::PlusZero,
                    Optimization::MultiplyByZero,
                ],
            ),
            ("x^0 + y*1", "1+y", vec![Optimization::PowerOfZero, Optimization::MultiplyByOne]),
            ("(a+b)+(c+d)", "a+b+c+d", vec![Optimization::FlattenPlus]),
            ("(a*b)*(c*d)", "a*b*c*d", vec![Optimization::FlattenMultiply]),
            ("a*a^-1", "1", vec![Optimization::DivideBySame]),
            ("33*(2*33)^-1", "2^-1", vec![Optimization::FlattenPower, Optimization::FlattenMultiply]),
        ];

        // Führe alle Testfälle durch
        for (input, expected_output, expected_optimizations) in test_cases {
            let mut formula = Element::parse(input).unwrap();
            let optimizations = formula.optimize_all();
            let expected = Element::parse(expected_output).unwrap();

            // Überprüfe Endergebnis
            assert_eq!(formula, expected, "Fehler bei Formel: {}", input);

            // Prüfe, ob erwartete Optimierungen durchgeführt wurden
            for opt in &expected_optimizations {
                assert!(
                    optimizations.contains(opt),
                    "Optimierung {:?} wurde nicht angewendet für: {}",
                    opt,
                    input
                );
            }

            // Prüfe, dass mindestens eine Optimierung durchgeführt wurde
            assert!(!optimizations.is_empty(), "Keine Optimierungen für: {}", input);
        }
    }
}
