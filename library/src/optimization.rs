use crate::expression_values::{CustomFunction, ExpressionFunType, ExpressionNumType, FunctionExpression};
use crate::formula_short::{fun_expr, inv, mul, num, num_expr};
use crate::parsing::signature::ParamCount;
use crate::{Element, FormulaStore, Number, create_default_context, formula};
use macros::formula_matches;
use num_traits::{Signed, ToPrimitive};
use std::cmp::PartialEq;
use std::mem;
use std::ops::Mul;
use std::rc::Rc;
use strum::{EnumCount, IntoEnumIterator};
use strum_macros::{EnumCount, EnumIter};

macro_rules! quick_match {
    ($input:expr,$pat:pat => $expr:expr) => {
        match $input {
            $pat => Some($expr),
            _ => None,
        }
    };
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

impl Element {
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
            | Element::NumberWithExpression { .. } => false,
        }
    }

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
                elements.retain(|e| !formula_matches!(e, num(0)));

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
                let mut outer_neg = false;
                for e in elements.iter_mut() {
                    if let Element::Negate(inner) = e {
                        outer_neg ^= true;
                        *e = mem::replace(inner, Element::Number(Number::nan(None)));
                    }
                }
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
                elements.retain(|e| !formula_matches!(e, num(1)));

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
                    *self = if outer_neg { formula!(neg(replacement)) } else { replacement }
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
                    if num == 1 {
                        *self = formula!(num(num.clone()));
                    } else if num == 0 {
                        *self = fun_expr(
                            ExpressionFunType::Custom(CustomFunction::new(
                                "assert_not_negative",
                                ParamCount::Exactly(1),
                                FunctionExpression::SingleArgument(Rc::new(|im, _ctx| {
                                    if im.is_negative() { Number::nan(None) } else { Number::from(0) }
                                })),
                            )),
                            [exp.as_ref().clone()],
                        )
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
            Element::FunctionWithExpression { arguments, expr_value } => match expr_value {
                ExpressionFunType::Floor | ExpressionFunType::Round => {
                    if arguments.len() == 1
                        && formula_matches!(arguments[0], num(x)).is_some_and(|n| n.is_integer())
                    {
                        *self = arguments[0].clone();
                        return;
                    }
                },
                ExpressionFunType::Sin => {
                    if arguments.len() == 1 {
                        let arg = &arguments[0];
                        let divided = mul([arg.clone(), inv(num_expr(ExpressionNumType::Pi))]);
                        let mut rem = fun_expr(ExpressionFunType::Rem, [divided.clone(), num(2)]);
                        rem.optimize_new();
                        let corrected =
                            fun_expr(ExpressionFunType::SinWithRadians, [mul([rem.clone(), num(2)])]);
                        *self = corrected;
                        return;
                    }
                },
                _ => {},
            },
            Element::Function { .. }
            | Element::Variable(_)
            | Element::VariableOrFunction(_)
            | Element::NumberWithExpression { .. }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_short::{inv, mul, num, var};
    use crate::{FormulaStore, create_default_context};

    #[test]
    fn test_optimize_new() {
        use crate::formula_short::{inv, mul, num, var};

        // doppelte Negation
        test(formula!(neg(neg(var("a")))), var("a"));

        // Plus entfernt Nullen komplett
        test(formula!(plus(num(0), num(0))), num(0));

        // Plus reduziert auf einzelnes Element nach Nullentfernung
        test(formula!(plus(num(0), var("a"), num(0))), var("a"));

        // Plus mit additivem Inversen ergibt 0
        test(formula!(plus(var("a"), neg(var("a")))), num(0));

        // Plus mit additiven Inversen und weiterem Term
        test(formula!(plus(var("a"), neg(var("a")), var("b"))), var("b"));

        // Plus mit mehreren Paaren
        test(formula!(plus(var("a"), neg(var("a")), var("b"), var("c"), neg(var("c")))), var("b"));

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
        test(formula!(mul(var("a"), num(0), var("b"))), num(0));

        // Multiply entfernt inverses Paar -> 1
        test(mul([var("a"), inv(var("a"))]), num(1));

        // Multiply entfernt inverses Paar und reduziert auf einzelnes Element
        test(formula!(mul(var("a"), var("b"), inv(var("a")))), var("b"));

        // Multiply mit 0 dominiert trotz inverser Faktoren
        test(formula!(mul(num(0), var("a"), inv(var("a")))), num(0));

        // Potenz Basis 1
        test(formula!(pow(num(1), var("x"))), num(1));

        // Potenz Basis 0
        test(formula!(pow(num(0), var("x"))), num(0));

        // Exponent 1
        test(formula!(pow(var("a"), num(1))), var("a"));

        // Zusammenführen verschachtelter Exponenten
        test(formula!(pow(pow(var("a"), num(2)), num(3))), formula!(pow(var("a"), mul(num(2), num(3)))));

        // Mehrfach verschachtelte Exponenten
        test(
            formula!(pow(pow(pow(var("a"), num(2)), num(3)), num(4))),
            formula!(pow(var("a"), mul(num(2), num(3), num(4)))),
        );

        // Exponent 1 verhindert weiteres Kombinieren
        test(formula!(pow(pow(var("a"), num(2)), num(1))), formula!(pow(var("a"), num(2))));

        // -(-a)*1 + (0+b) + c*(d*0) => a+b,
        let mut f = formula!(plus(
            mul(neg(neg(var("a"))), num(1)),
            plus(num(0), var("b")),
            mul(var("c"), mul(var("d"), num(0)))
        ));
        f.optimize_new();
        assert_eq!(f, formula!(plus(var("a"), var("b"))));

        // x^0 + y*1 => 1 + y
        test(formula!(plus(pow(var("x"), num(0)), mul(var("y"), num(1)))), formula!(plus(num(1), var("y"))));

        // (a+b)+(c+d) -> a+b+c+d
        test(
            formula!(plus(plus(var("a"), var("b")), plus(var("c"), var("d")))),
            formula!(plus(var("a"), var("b"), var("c"), var("d"))),
        );

        // (a*b)*(c*d) -> a*b*c*d
        test(
            formula!(mul(mul(var("a"), var("b")), mul(var("c"), var("d")))),
            formula!(mul(var("a"), var("b"), var("c"), var("d"))),
        );

        test(formula!(mul(var("a"), inv(var("a")))), num(1));

        test(
            formula!(mul(num(33), pow(mul(num(2), num(33)), neg(num(1))))),
            formula!(pow(num(2), neg(num(1)))),
        );

        test(formula!(mul(neg(num(2)))), formula!(neg(num(2))));
        test(formula!(mul(neg(num(2)), neg(num(2)))), formula!(mul(num(2), num(2))));

        test(formula!(mul(num(0), pow(num(0), neg(num(1))))), Element::Number(Number::nan(None)));
    }

    fn test(mut input: Element, expected: Element) {
        input.optimize_new();
        assert_eq!(input, expected);
    }

    #[test]
    fn test_expr_fun_optimization() {
        // pi optimization
        let mut fs = FormulaStore::new_empty();
        let ctx = &mut create_default_context();
        fs.define_default_symbols().unwrap();
        assert_eq!(fs.eval("sin(2*pi)", ctx), Ok(Number::from(0)));
        assert_eq!(fs.eval("sin(-2*pi)", ctx), Ok(Number::from(0)));
        assert_eq!(fs.eval("sin(10*pi)", ctx), Ok(Number::from(0)));
        assert_eq!(fs.eval("sin(pi)", ctx), Ok(Number::from(0)));
        assert_eq!(fs.eval("sin(-pi)", ctx), Ok(Number::from(0)));
        assert_eq!(fs.eval("sin(pi/2)", ctx), Ok(Number::from(1)));
        assert_eq!(fs.eval("sin(-pi/2)", ctx), Ok(Number::from(-1)));
        assert_eq!(fs.eval("sin(pi/6)", ctx), Ok(Number::from_string("0.5").unwrap()));
        assert_eq!(fs.eval("sin(-pi/6)", ctx), Ok(Number::from_string("-0.5").unwrap()));
        assert_eq!(fs.eval("sin(pi/3)", ctx), Ok(fs.eval("sqrt(3)/2", ctx).unwrap()));
    }
}
