use crate::Element;
use colored::Colorize;

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
    fn print(self, output: &mut String, show_type: bool) {
        match self {
            Inner::Single(element) => element.add_to_string(true, show_type, output),
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
                    element.add_to_string(true, show_type, output);
                }
            },
        }
    }
}

fn print_in_brackets<'a, T: IntoIterator<Item = &'a Element>>(
    inner: Inner<'a, T>, show_brackets: bool, string_before_brackets: Option<&str>,
    show_types: bool, type_string: &str, output: &mut String,
) {
    add_type_string(show_types, type_string, output);
    if !show_brackets {
        inner.print(output, show_types);
    } else {
        if let Some(str) = string_before_brackets {
            output.push_str(str);
        }
        output.push_str("(");
        inner.print(output, show_types);
        output.push_str(")");
    }
}

fn mark_string_red(str: impl ToString, apply_color: bool) -> String {
    if apply_color { str.to_string().red().to_string() } else { str.to_string() }
}

impl Element {
    pub fn print_debug(&self) {
        let mut string = String::new();
        self.add_to_string(true, true, &mut string);
        println!("{}", string);
    }
    pub fn print(&self) {
        let mut string = String::new();
        self.add_to_string(false, false, &mut string);
        println!("{}", string);
    }

    fn add_to_string(&self, show_brackets: bool, show_types: bool, output: &mut String) {
        match self {
            Element::Brackets(elements) => print_in_brackets(
                Inner::Multiple { delimiter: if show_types { "," } else { "" }, elements },
                show_brackets,
                None,
                show_types,
                "br",
                output,
            ),
            Element::Plus(elements) => print_in_brackets(
                Inner::Multiple { delimiter: "+", elements },
                show_brackets,
                None,
                show_types,
                "plus",
                output,
            ),
            Element::Multiply(elements) => print_in_brackets(
                Inner::Multiple { delimiter: "*", elements },
                show_brackets,
                None,
                show_types,
                "mul",
                output,
            ),
            Element::Function { name, arguments } => print_in_brackets(
                Inner::Multiple { delimiter: ",", elements: arguments },
                show_brackets,
                Some(name),
                show_types,
                "fun",
                output,
            ),
            Element::Pow(base, exponent) => {
                print_in_brackets(
                    Inner::Multiple { delimiter: "^", elements: [base.as_ref(), exponent] },
                    show_brackets,
                    None,
                    show_types,
                    "pow",
                    output,
                );
            },
            Element::Negate(element) => {
                add_element_string(show_types, "neg", "-", output);
                element.add_to_string(true, show_types, output);
            },
            Element::Number(num) => add_element_string(show_types, "num", num, output),
            Element::Variable(name) => add_element_string(show_types, "var", name, output),
            Element::VariableOrFunction(name) => {
                add_element_string(show_types, "var or fun", name, output)
            },
            Element::String(s) => {
                add_element_string(show_types, "str", mark_string_red(s, true), output)
            },
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
