use crate::expression_values::{ExprValue, ExpressionFunType, ExpressionNumType};
use crate::formula_short::{fun_expr_1_arg, fun_expr_n_args, fun_expr_new, inv, mul, neg, num, num_expr};
use crate::parsing::signature::ParamCount;
use crate::{Element, FormulaStore, FunctionExpression, Number, formula};
use macros::formula_matches;
use num_rational::BigRational;
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

    /// Checks if the elements are identical or their expression values as well as arguments are equal
    pub(crate) fn same_value(&self, other: &Element) -> bool {
        self == other
            || match (self, other) {
            (
                Element::FunctionWithExpression { arguments, expr_value: debug_name, .. },
                Element::FunctionWithExpression {
                    arguments: other_args,
                    expr_value: other_debug_name,
                    ..
                },
            ) => {
                debug_name.same_value(other_debug_name)
                    && arguments.len() == other_args.len()
                    && arguments.iter().zip(other_args).all(|(a, b)| a.same_value(b))
            },
            (
                Element::NumberWithExpression { expr_value: debug_name, .. },
                Element::NumberWithExpression { expr_value: other_debug_name, .. },
            ) => debug_name.same_value(other_debug_name),
            _ => false,
        }
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
            Element::FunctionWithExpression { arguments, expr_value, .. } => {
                if let ExprValue::Native(fun_type) = expr_value {
                    match fun_type {
                        ExpressionFunType::Floor | ExpressionFunType::Round => {
                            if arguments.len() == 1
                                && arguments[0].get_number_inner().is_some_and(|n| n.is_integer())
                            {
                                *self = arguments[0].clone();
                                return;
                            }
                        },
                        ExpressionFunType::Sin => {
                            if arguments.len() == 1 {
                                let arg = &arguments[0];
                                let divided = mul([arg.clone(), inv(num_expr(ExpressionNumType::Pi))]);
                                let rem = fun_expr_n_args(ExpressionFunType::Rem, [divided.clone(), num(2)]);
                                let corrected = fun_expr_new(
                                    "negative_over_1",
                                    ParamCount::Exactly(1),
                                    FunctionExpression::SingleArgument(|num, ctx| {
                                        if num
                                            .get_exact_rational()
                                            .is_some_and(|r| r >= BigRational::from_integer(1.into()))
                                        {
                                            num.plus(&Number::from(-1), ctx).neg(ctx)
                                        } else {
                                            num.clone()
                                        }
                                    }),
                                    [rem.clone()],
                                );
                                let mut multiplied = mul([corrected, num_expr(ExpressionNumType::Pi)]);
                                multiplied.optimize_new();
                                *self = fun_expr_1_arg(ExpressionFunType::Sin, [multiplied]);
                                return;
                            }
                        },
                        _ => {},
                    }
                }
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
    use crate::formula_short::{inv, mul, num, var};
    use crate::{FormulaStore, create_default_context};

    #[test]
    fn test_optimize_new() {
        use crate::formula_short::{inv, mul, num, var};

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

    #[test]
    fn test_expr_fun_optimization() {
        // pi optimization
        let mut fs = FormulaStore::new_empty();
        let ctx = &mut create_default_context();
        fs.define_default_symbols().unwrap();
        assert_eq!(fs.eval("sin(2*pi)", ctx), Ok(Number::from(0)));
        assert_eq!(fs.eval("sin(10*pi)", ctx), Ok(Number::from(0)));
        assert_eq!(fs.eval("sin(pi)", ctx), Ok(Number::from(0)));
        assert_eq!(fs.eval("sin(pi/2)", ctx), Ok(Number::from(1)));
        assert_eq!(fs.eval("sin(pi/6)", ctx), Ok(Number::from_string("0.5").unwrap()));
        assert_eq!(fs.eval("sin(pi/3)", ctx), Ok(fs.eval("sqrt(3)/2", ctx).unwrap()));
    }
}
