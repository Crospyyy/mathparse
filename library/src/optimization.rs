use crate::{Element, Number};
use std::cmp::PartialEq;
use strum::{EnumCount, IntoEnumIterator};
use strum_macros::{EnumCount, EnumIter};

#[derive(EnumIter, EnumCount, Copy, Clone, PartialEq)]
pub enum Optimization {
    CombineExponents,
    OneToAnyPower,
    MultiplyByOne,
    PlusZero,
    DoubleNegation,
    DivideBySame,
    MultiplyByZero,
    FlattenPlus,
    FlattenMultiply,
}

impl Element {
    pub fn optimize_all(&mut self) -> Vec<Optimization> {
        let mut all_optimizations = Vec::with_capacity(Optimization::COUNT);
        for optimization in Optimization::iter() {
            if self.optimize(optimization) {
                all_optimizations.push(optimization);
            }
        }
        all_optimizations
    }

    fn optimize(&mut self, optimization: Optimization) -> bool {
        if self.anything_unparsed() {
            return false;
        }

        let mut successful = self.run_on_children(&mut |element| Self::optimize(element, optimization));
        match optimization {
            Optimization::CombineExponents => {
                if let Element::Pow(base, exponent) = self {
                    if let Element::Pow(base_inner, exponent_inner) = base.as_mut() {
                        *self = Element::Pow(
                            Box::new(*base_inner.clone()),
                            Box::new(Element::Multiply(vec![*exponent_inner.clone(), *exponent.clone()])),
                        );
                        successful = true;
                    }
                }
            },
            Optimization::OneToAnyPower => {
                if let Element::Pow(base, _) = self {
                    if let Element::Number(b) = &**base {
                        if *b == Number::from(1) {
                            *self = Element::Number(Number::from(1));
                            successful = true;
                        }
                    }
                }
            },
            Optimization::MultiplyByOne => {
                if let Element::Multiply(elements) = self {
                    let len_before = elements.len();
                    elements.retain(
                        |e| {
                            if let Element::Number(n) = e { *n != Number::from(1) } else { true }
                        },
                    );
                    successful |= len_before != elements.len();
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
                    elements.retain(
                        |e| {
                            if let Element::Number(n) = e { *n != Number::from(0) } else { true }
                        },
                    );
                    successful |= len_before != elements.len();
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
                        successful = true;
                    }
                }
            },
            Optimization::DivideBySame => {
                fn b_is_inverse_of_a(a: &Element, b: &Element) -> bool {
                    match (a, b) {
                        (a, Element::Pow(base_b, exp_b)) => {
                            if let Element::Negate(n) = &**exp_b {
                                if let Element::Number(n) = &**n {
                                    *n == Number::from(1) && *a == **base_b
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        },
                        _ => false,
                    }
                }
                if let Element::Multiply(elements) = self {
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
                        successful = true;
                    }
                }
            },
            Optimization::MultiplyByZero => {},
            Optimization::FlattenPlus => {},
            Optimization::FlattenMultiply => {},
        }
        successful
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_short::{inv, mul, num, var};

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

    #[test]
    fn test_combine_exponents() {
        check_input_and_output_match("(a^2)^3", "a^(2*3)", Optimization::CombineExponents);
    }

    #[test]
    fn test_one_to_any_power() {
        check_input_and_output_match("1^-1", "1", Optimization::OneToAnyPower);
        check_input_and_output_match("1^x", "1", Optimization::OneToAnyPower);
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
        check_input_and_output_match("a*(b*c)", "a*b*c", Optimization::FlattenMultiply);
    }

    #[test]
    fn test_optimize_all() {
        let mut formula = Element::parse("-(-a)*1 + 0").unwrap();

        let success1 = formula.optimize(Optimization::DoubleNegation);
        let success2 = formula.optimize(Optimization::MultiplyByOne);
        let success3 = formula.optimize(Optimization::PlusZero);

        assert!(success1 && success2 && success3);
        assert_eq!(formula, Element::parse("a").unwrap());
    }
}
