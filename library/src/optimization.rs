use crate::{Element, Number, formula};
use astro_float::Error;
use macros::formula_matches;
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

impl Element {
    pub fn optimize_all(&mut self) -> Vec<Optimization> {
        let mut all_optimizations = Vec::new();
        loop {
            let mut any_optimization = false;
            for optimization in Optimization::iter() {
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

impl Element {
    fn is_negative_of(&self, other: &Element) -> bool {
        self.get_negate_inner().is_some_and(|n| n == other)
            || other.get_negate_inner().is_some_and(|n| n == self)
    }
}

fn flatten_list<T: Clone>(list: &mut Vec<T>, flatten_fn: fn(&mut T) -> Option<&mut Vec<T>>) {
    let mut new_list = vec![];
    for e in list.iter_mut() {
        if let Some(inner) = flatten_fn(e) {
            flatten_list(inner, flatten_fn);
            new_list.append(inner);
        } else {
            new_list.push(e.clone());
        }
    }
    *list = new_list;
}

macro_rules! quick_match {
    ($input:expr,$pat:pat => $expr:expr) => {
        match $input {
            $pat => Some($expr),
            _ => None,
        }
    };
}

impl Element {
    pub(crate) fn optimize_new(&mut self) {
        self.run_on_children(&mut |child| {
            child.optimize_new();
            false
        });
        match self {
            Element::Plus(elements) => {
                if elements.iter().any(|e| formula_matches!(e, plus)) {
                    flatten_list(elements, |e| quick_match!(e, Element::Plus(inner) => inner))
                }
                if let Some(e) = elements.iter().find(|e| e.is_nan()) {
                    *self = e.clone();
                    return;
                }
                elements.retain(|e| !e.is(0));

                if elements.len() >= 2 {
                    let mut to_remove = vec![false; elements.len()];
                    'outer: for i in 0..elements.len() - 1 {
                        for j in i + 1..elements.len() {
                            if to_remove[j] {
                                continue;
                            }
                            if formula_matches!(elements[i], neg({ &elements[j] }))
                                || formula_matches!(elements[j], neg({ &elements[i] }))
                            {
                                to_remove[i] = true;
                                to_remove[j] = true;
                                continue 'outer;
                            }
                        }
                    }
                    for (i, _) in to_remove.iter().copied().enumerate().filter(|x| x.1).rev() {
                        elements.remove(i);
                    }
                }

                if let Some(replacement) = Self::handle_empty_or_one_element(elements, 0) {
                    *self = replacement
                }
            },
            Element::Multiply(elements) => {
                if elements.iter().any(|e| formula_matches!(e, mul)) {
                    flatten_list(elements, |e| quick_match!(e, Element::Multiply(inner) => inner))
                }
                if let Some(e) = elements.iter().find(|e| e.is_nan()) {
                    *self = e.clone();
                    return;
                }
                if elements.iter().any(|e| formula_matches!(e, num(0))) {
                    *self = formula!(num(0));
                    return;
                }
                elements.retain(|e| !e.is(1));

                if elements.len() >= 2 {
                    let mut to_remove = vec![false; elements.len()];
                    'outer: for i in 0..elements.len() - 1 {
                        for j in i + 1..elements.len() {
                            if to_remove[j] {
                                continue;
                            }
                            if formula_matches!(elements[i], pow({ &elements[j] }, neg(num(1))))
                                || formula_matches!(elements[j], pow({ &elements[i] }, neg(num(1))))
                            {
                                to_remove[i] = true;
                                to_remove[j] = true;
                                continue 'outer;
                            }
                        }
                    }
                    for (i, _) in to_remove.iter().copied().enumerate().filter(|x| x.1).rev() {
                        elements.remove(i);
                    }
                }

                if let Some(replacement) = Self::handle_empty_or_one_element(elements, 1) {
                    *self = replacement
                }
            },
            Element::Negate(n) => {
                if let Element::Negate(e) = n.as_ref() {
                    *self = e.as_ref().clone();
                    return;
                }
            },
            Element::Pow(base, exp) => {
                if let Some(num) = formula_matches!(base.as_ref(), num(x)) {
                    if num == 1 || num == 0 {
                        *self = formula!(num(num.clone()));
                    }
                    return;
                }
                if let Some(x) = formula_matches!(exp.as_ref(), num(x)) {
                    if x == 1 {
                        *self = base.as_ref().clone();
                        return;
                    }
                    if x == 0 {
                        *self = formula!(num(1));
                        return;
                    }
                }
                if let Some(elements) = quick_match!(base.as_mut(), Element::Multiply(inner) => inner) {
                    for element in elements.iter_mut() {
                        let exp = exp.as_ref().clone();
                        *element = formula!(pow(element, exp))
                    }
                    *self = Element::Multiply(elements.clone());
                    self.optimize_new();
                    return;
                }
                if let Some((inner_base, inner_exp)) = formula_matches!(base.as_ref(), pow(x, x)) {
                    let exp_ref = exp.as_ref();
                    *self = formula!(pow(inner_base, mul(inner_exp, exp_ref)));
                    self.optimize_new();
                }
            },
            Element::Function { .. }
            | Element::Variable(_)
            | Element::VariableOrFunction(_)
            | Element::FunctionWithExpression { .. }
            | Element::NumberWithExpression(_)
            | Element::Number(_)
            | Element::Brackets(_)
            | Element::String(_) => {},
        }
    }

    fn handle_empty_or_one_element(elements: &Vec<Element>, neutral_element: u16) -> Option<Element> {
        if elements.is_empty() {
            Some(formula!(num(neutral_element as i32)))
        } else if elements.len() == 1 {
            Some(elements[0].clone())
        } else {
            None
        }
    }
}

macro_rules! implement_internal {
    ($name:ident, $name_mut:ident, $return_type:ty, $enum_name:ident, $inner_name:ident) => {
        pub(crate) fn $name(&self) -> Option<&$return_type> {
            match self {
                Element::$enum_name($inner_name) => Some($inner_name),
                _ => None,
            }
        }

        pub(crate) fn $name_mut(&mut self) -> Option<&mut $return_type> {
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

    #[test]
    fn test_optimize_new() {
        use crate::formula_short::{inv, mul, neg, num, plus, pow, var};

        // doppelte Negation
        let mut f = formula!(neg(neg(var("a"))));
        f.optimize_new();
        assert_eq!(f, var("a"));

        // Plus entfernt Nullen komplett
        let mut f = formula!(plus(num(0), num(0)));
        f.optimize_new();
        assert_eq!(f, num(0));

        // Plus reduziert auf einzelnes Element nach Nullentfernung
        let mut f = formula!(plus(num(0), var("a"), num(0)));
        f.optimize_new();
        assert_eq!(f, var("a"));

        // Plus mit additivem Inversen ergibt 0
        let mut f = formula!(plus(var("a"), neg(var("a"))));
        f.optimize_new();
        assert_eq!(f, num(0));

        // Plus mit additiven Inversen und weiterem Term
        let mut f = formula!(plus(var("a"), neg(var("a")), var("b")));
        f.optimize_new();
        assert_eq!(f, var("b"));

        // Plus mit mehreren Paaren
        let mut f = formula!(plus(var("a"), neg(var("a")), var("b"), var("c"), neg(var("c"))));
        f.optimize_new();
        assert_eq!(f, var("b"));

        // Plus propagiert NaN
        let nan_el = Element::Number(Number::nan(None));
        let mut f = formula!(plus(var("x"), nan_el, var("y")));
        f.optimize_new();
        assert!(f.is_nan());

        // Multiply propagiert NaN
        let nan_el2 = Element::Number(Number::nan(None));
        let mut f = mul([var("x"), nan_el2.clone(), var("y")]);
        f.optimize_new();
        assert!(f.is_nan());

        // Multiply mit 0 ergibt 0
        let mut f = formula!(mul(var("a"), num(0), var("b")));
        f.optimize_new();
        assert_eq!(f, num(0));

        // Multiply entfernt inverses Paar -> 1
        let mut f = mul([var("a"), inv(var("a"))]);
        f.optimize_new();
        assert_eq!(f, num(1));

        // Multiply entfernt inverses Paar und reduziert auf einzelnes Element
        let mut f = formula!(mul(var("a"), var("b"), inv(var("a"))));
        f.optimize_new();
        assert_eq!(f, var("b"));

        // Multiply mit 0 dominiert trotz inverser Faktoren
        let mut f = formula!(mul(num(0), var("a"), inv(var("a"))));
        f.optimize_new();
        assert_eq!(f, num(0));

        // Potenz Basis 1
        let mut f = formula!(pow(num(1), var("x")));
        f.optimize_new();
        assert_eq!(f, num(1));

        // Potenz Basis 0
        let mut f = formula!(pow(num(0), var("x")));
        f.optimize_new();
        assert_eq!(f, num(0));

        // Exponent 1
        let mut f = formula!(pow(var("a"), num(1)));
        f.optimize_new();
        assert_eq!(f, var("a"));

        // Zusammenführen verschachtelter Exponenten
        let mut f = formula!(pow(pow(var("a"), num(2)), num(3)));
        f.optimize_new();
        assert_eq!(f, formula!(pow(var("a"), mul(num(2), num(3)))));

        // Mehrfach verschachtelte Exponenten
        let mut f = formula!(pow(pow(pow(var("a"), num(2)), num(3)), num(4)));
        f.optimize_new();
        assert_eq!(f, formula!(pow(var("a"), mul(num(2), num(3), num(4)))));

        // Exponent 1 verhindert weiteres Kombinieren
        let mut f = formula!(pow(pow(var("a"), num(2)), num(1)));
        f.optimize_new();
        assert_eq!(f, formula!(pow(var("a"), num(2))));

        // -(-a)*1 + (0+b) + c*(d*0) => a+b,
        let mut f = formula!(plus(
            mul(neg(neg(var("a"))), num(1)),
            plus(num(0), var("b")),
            mul(var("c"), mul(var("d"), num(0)))
        ));
        f.optimize_new();
        assert_eq!(f, formula!(plus(var("a"), var("b"))));

        // x^0 + y*1 => 1 + y
        let mut f = formula!(plus(pow(var("x"), num(0)), mul(var("y"), num(1))));
        f.optimize_new();
        assert_eq!(f, formula!(plus(num(1), var("y"))));

        // (a+b)+(c+d) -> a+b+c+d
        let mut f = formula!(plus(plus(var("a"), var("b")), plus(var("c"), var("d"))));
        f.optimize_new();
        assert_eq!(f, formula!(plus(var("a"), var("b"), var("c"), var("d"))));

        // (a*b)*(c*d) -> a*b*c*d
        let mut f = formula!(mul(mul(var("a"), var("b")), mul(var("c"), var("d"))));
        f.optimize_new();
        assert_eq!(f, formula!(mul(var("a"), var("b"), var("c"), var("d"))));

        // a*a^-1 -> 1 (bereits oben getestet, hier nochmal aus Sammlung)
        let mut f = formula!(mul(var("a"), inv(var("a"))));
        f.optimize_new();
        assert_eq!(f, num(1));

        // 33*(2*33)^-1 bleibt unverändert da entsprechende Optimierung fehlt
        let mut f = formula!(mul(num(33), pow(mul(num(2), num(33)), neg(num(1)))));
        f.optimize_new();
        assert_eq!(f, formula!(pow(num(2), neg(num(1)))));
    }
}
