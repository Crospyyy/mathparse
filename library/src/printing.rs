use crate::{Element, FunctionExpression};
use astro_float::ctx::Context;
use colored::Colorize;
use std::fmt::Display;

enum Inner<'a, T: 'a>
where
    T: IntoIterator<Item = &'a Element>,
{
    #[allow(unused)]
    Single(&'a Element),
    Multiple {
        delimiter: &'a str,
        elements: T,
    },
}

impl<'a, T> Inner<'a, T>
where
    T: IntoIterator<Item = &'a Element>,
{
    fn print(self, output: &mut String, show_type: bool, ctx: &mut Context) {
        match self {
            Inner::Single(element) => element.add_to_string(true, show_type, output, ctx),
            Inner::Multiple { delimiter, elements } => {
                for (i, element) in elements.into_iter().enumerate() {
                    if i != 0 {
                        if show_type {
                            output.push(' ');
                        }
                        output.push_str(delimiter);
                        if show_type {
                            output.push(' ');
                        }
                    }
                    element.add_to_string(true, show_type, output, ctx);
                }
            },
        }
    }
}

fn print_in_brackets<'a, T: IntoIterator<Item = &'a Element>>(
    inner: Inner<'a, T>, show_brackets: bool, string_before_brackets: Option<&str>, show_types: bool,
    type_string: &str, output: &mut String,
    ctx: &mut Context,
) {
    add_type_string(show_types, type_string, output);
    if !show_brackets {
        inner.print(output, show_types, ctx);
    } else {
        if let Some(str) = string_before_brackets {
            output.push_str(str);
        }
        output.push_str("(");
        inner.print(output, show_types, ctx);
        output.push_str(")");
    }
}

fn mark_string_red(str: impl ToString, apply_color: bool) -> String {
    if apply_color { str.to_string().red().to_string() } else { str.to_string() }
}

// impl Display for Element {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let mut string = String::new();
//         self.add_to_string(false, false, &mut string);
//         write!(f, "{}", string)
//     }
// }

impl Element {
    pub fn get_string(&self, ctx: &mut Context) -> String {
        let mut output = String::new();
        self.add_to_string(false, false, &mut output, ctx);
        output
    }
    
    pub fn get_debug_string(&self) -> String {
        match self {
            Element::Brackets(e) => {
                format!("br({})", e.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", "))
            },
            Element::Plus(e) => {
                format!("add({})", e.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", "))
            },
            Element::Multiply(e) => {
                format!("mul({})", e.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", "))
            },
            Element::Pow(a, b) => format!("pow({}, {})", a.get_debug_string(), b.get_debug_string()),
            Element::String(s) => format!("\"{s}\""),
            Element::Negate(e) => format!("neg({})", e.get_debug_string()),
            Element::Number(n) => format!("num({})", n.get_debug_string()),
            Element::Function { name, arguments } => {
                format!(
                    "fun({name}, [{}])",
                    arguments.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", ")
                )
            },
            Element::Variable(name) => format!("var({})", name),
            Element::VariableOrFunction(name) => {
                format!("var_or_fun({})", name)
            },
            Element::FunctionWithExpression { arguments, expression } => {
                let name_str = if matches!(expression, FunctionExpression::SingleArgument(_)) {
                    "one argument"
                } else {
                    "n arguments"
                };
                format!(
                    "fun_with_expr({}, [{}])",
                    name_str,
                    arguments.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", ")
                )
            },
            Element::NumberWithExpression(_) => {
                format!("num_with_expr({})", self.get_debug_string())
            },
        }
    }

    // pub fn print(&self) {
    //     println!("{}", self);
    // }

    fn add_to_string(&self, show_brackets: bool, show_types: bool, output: &mut String, ctx: &mut Context) {
        match self {
            Element::Brackets(elements) => print_in_brackets(
                Inner::Multiple { delimiter: if show_types { "," } else { "" }, elements },
                show_brackets,
                None,
                show_types,
                "br",
                output,
                ctx,
            ),
            Element::Plus(elements) => print_in_brackets(
                Inner::Multiple { delimiter: "+", elements },
                show_brackets,
                None,
                show_types,
                "plus",
                output,
                ctx,
            ),
            Element::Multiply(elements) => print_in_brackets(
                Inner::Multiple { delimiter: "*", elements },
                show_brackets,
                None,
                show_types,
                "mul",
                output,
                ctx,
            ),
            Element::Function { name, arguments } => print_in_brackets(
                Inner::Multiple { delimiter: ",", elements: arguments },
                show_brackets,
                Some(name),
                show_types,
                "fun",
                output,
                ctx,
            ),
            Element::FunctionWithExpression { arguments, .. } => print_in_brackets(
                Inner::Multiple { delimiter: ",", elements: arguments },
                show_brackets,
                Some("fun_with_expr"),
                show_types,
                "fun_with_expr",
                output,
                ctx,
            ),
            Element::Pow(base, exponent) => {
                print_in_brackets(
                    Inner::Multiple { delimiter: "^", elements: [base.as_ref(), exponent] },
                    show_brackets,
                    None,
                    show_types,
                    "pow",
                    output,
                    ctx,
                );
            },
            Element::Negate(element) => {
                add_element_string(show_types, "neg", "-", output);
                element.add_to_string(true, show_types, output, ctx);
            },
            Element::Number(num) => add_element_string(show_types, "num", num.to_string(ctx), output),
            Element::Variable(name) => add_element_string(show_types, "var", name, output),
            Element::VariableOrFunction(name) => add_element_string(show_types, "var or fun", name, output),
            Element::String(s) => add_element_string(show_types, "str", mark_string_red(s, true), output),
            Element::NumberWithExpression(_) => output.push_str("num_with_expr"),
        }
    }
}

fn add_element_string(
    show_types: bool, type_string: &str, content: impl std::fmt::Display, output: &mut String,
) {
    add_type_string(show_types, type_string, output);
    output.push_str(content.to_string().as_str());
}

fn add_type_string(show_types: bool, type_string: &str, output: &mut String) {
    if show_types {
        output.push_str(type_string);
        output.push_str(": ");
    }
}
