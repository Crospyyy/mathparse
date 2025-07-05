use crate::parsing::testing::test_with_user_input;

fn main() {
    test_with_user_input()
}

mod parsing {
    pub mod implementation {
        use colored::Colorize;
        use regex::Regex;
        use std::borrow::Cow;
        use std::mem;

        #[derive(Debug, Clone)]
        pub enum Element {
            Brackets(Vec<Element>),
            Plus(Vec<Element>),
            Multiply(Vec<Element>),
            Negate(Box<Element>),
            String(String),
            Variable(String),
            Number(f64),
            Pow(Box<Element>, Box<Element>),
        }

        impl Element {
            pub fn parse(input: &str) -> Self {
                let cow = Element::preprocess_string_minus(&input);
                let chars = cow.chars().collect::<Vec<_>>();
                let mut start = 0;
                let mut brackets = Element::bracketize(&chars, &mut start);
                brackets.process_plus();
                brackets.process_minus();
                brackets.process_multiply();
                brackets.process_divide();
                brackets.process_minus();
                brackets.process_pow();
                brackets.process_minus();
                brackets.process_numbers_and_variables();
                brackets
            }

            /// Step 0
            pub(super) fn preprocess_string_minus(input: &str) -> Cow<str> {
                let without_whitespace = input.replace(" ", "");
                Regex::new(r"([\w)])-([\w(])").unwrap().replace_all(&without_whitespace, "$1+-$2")
            }

            /// Step 1
            pub(super) fn bracketize(input: &[char], start: &mut usize) -> Element {
                let mut elements = Vec::new();
                let mut i = *start;
                while i < input.len() {
                    let char = input[i];
                    if !"()".contains(char) {
                        i += 1;
                        if i == input.len() && *start < i {
                            elements.push(Element::String(input[*start..i].iter().collect()));
                        }
                        continue;
                    }
                    if char == '(' {
                        if i > *start {
                            elements.push(Element::String(input[*start..i].iter().collect()));
                        }
                        i += 1; // ensure the pointer is behind the opening brackets
                        elements.push(Self::bracketize(&input, &mut i));
                        *start = i;
                    }
                    if char == ')' {
                        if i != *start {
                            elements.push(Element::String(input[*start..i].iter().collect()));
                        }
                        i += 1; // ensure that the pointer is behind the closing brackets
                        *start = i;
                        return Element::Brackets(elements);
                    }
                }
                *start = i;
                Element::Brackets(elements)
            }

            /// Step 2
            pub(super) fn process_plus(&mut self) {
                match self {
                    Element::Brackets(elements) => {
                        if let Some(elements) = split_list_by_char(elements, '+') {
                            *self = Element::Plus(elements);
                        }
                        if let Element::Brackets(elements) | Element::Plus(elements) = self {
                            elements.iter_mut().for_each(Element::process_plus);
                        }
                    },
                    Element::String(s) => {
                        if let Some(elements) = split_string_by_char(s, '+') {
                            *self = Element::Plus(elements);
                        }
                    },
                    _ => {},
                }
            }

            /// Steps 3, 5 and 7
            pub(super) fn process_minus(&mut self) {
                match self {
                    Element::Brackets(elements) => {
                        if let Some(Element::String(str)) = elements.first() {
                            if str.starts_with('-') {
                                elements[0] = Element::String(str[1..].to_owned());
                                *self = Element::Negate(Box::new(self.clone()));
                            }
                        } else {
                            if elements.len() == 1 {
                                elements[0].process_minus();
                            }
                        }
                        if let Element::Brackets(groups) = self {
                            groups.iter_mut().for_each(|e| match e {
                                Element::String(_) => {},
                                _ => e.process_minus(),
                            })
                        }
                    },
                    Element::Plus(elements) => {
                        elements.iter_mut().for_each(Element::process_minus);
                    },
                    Element::Multiply(elements) => {
                        elements.iter_mut().for_each(Element::process_minus);
                    },
                    Element::String(s) => {
                        if s.starts_with('-') {
                            *self = Element::Negate(Box::new(Element::String(s[1..].to_owned())));
                        }
                    },
                    Element::Negate(element) => {
                        element.process_minus();
                    },
                    Element::Pow(b, e) => {
                        b.process_minus();
                        e.process_minus();
                    },
                    _ => {},
                }
            }

            /// Step 4
            pub(super) fn process_multiply(&mut self) {
                match self {
                    Element::Brackets(elements) => {
                        if let Some(groups) = split_list_by_char(elements, '*') {
                            *self = Element::Multiply(groups);
                        }
                        if let Element::Brackets(elements) | Element::Multiply(elements) = self {
                            elements.iter_mut().for_each(Element::process_multiply);
                        }
                    },
                    Element::Plus(elements) => {
                        elements.iter_mut().for_each(Element::process_multiply)
                    },
                    Element::Negate(element) => element.process_multiply(),
                    Element::String(s) => {
                        if let Some(elements) = split_string_by_char(s, '*') {
                            *self = Element::Multiply(elements);
                        }
                    },
                    _ => {},
                }
            }

            /// Used in process_divide (Step 4)
            fn invert(&mut self) {
                *self = Element::Pow(
                    Box::new(self.clone()),
                    Box::new(Element::Negate(Box::new(Element::String("1".to_string())))),
                );
            }

            /// Step 4
            pub(super) fn process_divide(&mut self) {
                let create_divisions = |element: &mut Element, mut new_elements: Vec<Element>| {
                    new_elements[1..].iter_mut().for_each(Element::invert);
                    *element = Element::Multiply(new_elements);
                };
                match self {
                    Element::String(str) => {
                        if let Some(new_elements) = split_string_by_char(str, '/') {
                            create_divisions(self, new_elements);
                        }
                    },
                    Element::Brackets(elements) => {
                        if let Some(new_elements) = split_list_by_char(elements, '/') {
                            create_divisions(self, new_elements);
                        }
                        if let Element::Brackets(elements) | Element::Multiply(elements) = self {
                            elements.iter_mut().for_each(Element::process_divide);
                        }
                    },
                    Element::Plus(elements) => {
                        elements.iter_mut().for_each(Element::process_divide);
                    },
                    Element::Multiply(elements) => {
                        elements.iter_mut().for_each(Element::process_divide);
                    },
                    _ => {},
                }
            }

            /// Step 6
            pub(super) fn process_pow(&mut self) {
                let create_recursive_pow =
                    |element: &mut Element, mut new_elements: Vec<Element>| {
                        let mut working_element = new_elements.pop().unwrap();
                        for e in new_elements.into_iter().rev() {
                            working_element = Element::Pow(Box::new(e), Box::new(working_element));
                        }
                        *element = working_element;
                    };
                match self {
                    Element::Brackets(elements) => {
                        if let Some(new_elements) = split_list_by_char(elements, '^') {
                            create_recursive_pow(self, new_elements);
                        }
                        match self {
                            Element::Brackets(elements) => {
                                elements.iter_mut().for_each(Element::process_pow)
                            },
                            Element::Pow(b, e) => {
                                b.process_pow();
                                e.process_pow();
                            },
                            _ => {},
                        }
                    },
                    Element::Plus(elements) | Element::Multiply(elements) => {
                        elements.iter_mut().for_each(Element::process_pow);
                    },
                    Element::Negate(e) => e.process_pow(),
                    Element::String(s) => {
                        if let Some(new_elements) = split_string_by_char(s, '^') {
                            create_recursive_pow(self, new_elements);
                        }
                    },
                    Element::Pow(b, p) => {
                        b.process_pow();
                        p.process_pow();
                    },
                    _ => {},
                }
            }

            /// Step 8
            pub(super) fn process_numbers_and_variables(&mut self) {
                match self {
                    Element::String(s) => {
                        if let Ok(num) = s.parse::<f64>() {
                            *self = Element::Number(num);
                        } else {
                            // If parsing fails, we assume it's a variable
                            *self = Element::Variable(s.clone());
                        }
                    },
                    Element::Brackets(e) | Element::Multiply(e) | Element::Plus(e) => {
                        e.iter_mut().for_each(Element::process_numbers_and_variables);
                    },
                    Element::Negate(e) => e.process_numbers_and_variables(),
                    Element::Pow(base, exponent) => {
                        base.process_numbers_and_variables();
                        exponent.process_numbers_and_variables();
                    },
                    _ => {},
                }
            }

            pub fn print(&self) {
                self.debug_print(0, true, false);
                println!();
            }

            fn debug_print(&self, indent: usize, one_line: bool, inner_layer_call: bool) {
                fn print_in_brackets<F: FnOnce()>(
                    indent: usize, one_line: bool, inner_layer_call: bool, inner_print: F,
                ) {
                    if inner_layer_call {
                        Element::print_indented(indent, "(", one_line, false);
                    }
                    inner_print();
                    if inner_layer_call {
                        Element::print_indented(indent, ")", one_line, false);
                    }
                }
                match self {
                    Element::Brackets(elements) => {
                        print_in_brackets(indent, one_line, inner_layer_call, || {
                            for element in elements {
                                element.debug_print(indent + 1, one_line, true);
                            }
                        });
                    },
                    Element::Plus(elements) => {
                        print_in_brackets(indent, one_line, inner_layer_call, || {
                            for (i, element) in elements.iter().enumerate() {
                                element.debug_print(indent + 1, one_line, true);
                                if i + 1 < elements.len() {
                                    Self::print_indented(indent + 1, "+", one_line, false);
                                }
                            }
                        });
                    },
                    Element::Multiply(elements) => {
                        print_in_brackets(indent, one_line, inner_layer_call, || {
                            for (i, element) in elements.iter().enumerate() {
                                element.debug_print(indent + 1, one_line, true);
                                if i + 1 < elements.len() {
                                    Self::print_indented(indent + 1, "*", one_line, false);
                                }
                            }
                        });
                    },
                    Element::Negate(element) => {
                        Self::print_indented(indent, "-", one_line, false);
                        element.debug_print(indent + 1, one_line, true);
                    },
                    Element::String(s) => Self::print_indented(indent, s, one_line, true),
                    Element::Pow(base, exponent) => {
                        print_in_brackets(indent, one_line, inner_layer_call, || {
                            base.debug_print(indent + 1, one_line, true);
                            Self::print_indented(indent + 1, "^", one_line, false);
                            exponent.debug_print(indent + 1, one_line, true);
                        });
                    },
                    Element::Variable(name) => Self::print_indented(indent, name, one_line, false),
                    Element::Number(num) => {
                        Self::print_indented(indent, &num.to_string(), one_line, false)
                    },
                }
            }

            fn print_indented(indent: usize, str: &str, same_line: bool, color: bool) {
                let str = if color { str.red() } else { str.normal() };
                if same_line {
                    print!("{}", str);
                } else {
                    println!("{}{}", " ".repeat(indent * 4), str);
                }
            }
        }

        fn split_list_by_char(input: &[Element], delimiter: char) -> Option<Vec<Element>> {
            const DEBUG: bool = false;
            if DEBUG {
                println!("Processing list of elements: {:?}", input);
            }
            let any_delimiter = input
                .iter()
                .any(|e| if let Element::String(s) = e { s.contains(delimiter) } else { false });
            if !any_delimiter {
                if DEBUG {
                    println!("No '{delimiter}' found in brackets.");
                }
                return None;
            }

            if DEBUG {
                println!("Found '{delimiter}' in list, processing...");
            }
            let mut groups = Vec::new();
            let mut current_group = Vec::new();

            fn add_current_group(groups: &mut Vec<Element>, current_group: &mut Vec<Element>) {
                if !current_group.is_empty() {
                    let mut group_to_add = mem::replace(current_group, Vec::new());
                    if group_to_add.len() == 1 {
                        groups.push(group_to_add.pop().unwrap());
                    } else {
                        groups.push(Element::Brackets(group_to_add));
                    }
                }
            };
            for element in input {
                if let Element::String(str) = element {
                    if DEBUG {
                        println!("Processing string element: {:?}", str);
                    }
                    if !str.contains(delimiter) {
                        if DEBUG {
                            println!(
                                "String does not contain '{delimiter}', adding to current group."
                            );
                        }
                        current_group.push(element.clone());
                    } else {
                        let parts: Vec<&str> = str.split(delimiter).collect();
                        if DEBUG {
                            println!(
                                "String contains '{delimiter}', splitting into parts: {:?}",
                                parts
                            );
                        }
                        for (i, part) in parts.iter().enumerate() {
                            if i == 0 {
                                if part.is_empty() {
                                    add_current_group(&mut groups, &mut current_group);
                                } else {
                                    current_group.push(Element::String(part.to_string()));
                                }
                            } else if i > 0 {
                                add_current_group(&mut groups, &mut current_group);
                                if !part.is_empty() {
                                    current_group.push(Element::String(part.to_string()));
                                }
                            }
                        }
                    }
                } else {
                    if DEBUG {
                        println!("Processing non-string element: {:?}", element);
                    }
                    current_group.push(element.clone());
                }
            }

            add_current_group(&mut groups, &mut current_group);

            Some(groups)
        }

        fn split_string_by_char(input: &str, delimiter: char) -> Option<Vec<Element>> {
            if !input.contains(delimiter) {
                return None;
            }
            input
                .split(delimiter)
                .filter(|s| !s.is_empty())
                .map(|s| Element::String(s.to_string()))
                .collect::<Vec<_>>()
                .into()
        }
    }

    pub mod testing {
        use crate::parsing::implementation::Element;

        pub fn test_with_user_input() {
            use std::io::stdin;

            loop {
                println!("Enter a formula to parse (or '' to quit):");
                let mut input = String::new();
                stdin().read_line(&mut input).unwrap();
                let input = input.trim();

                if input.is_empty() {
                    return;
                }

                let element = Element::parse(input);
                print!("parsed formula: ");
                element.print();
                if let Some(num) = element.eval() {
                    println!("Result: {}", num);
                } else {
                    println!("Result: Could not evaluate the formula.");
                }
                println!();
            }
        }

        pub fn run_tests() {
            let inputs = ["((x/x-x)*-x^x)/(x-x)^-x", "(x/x+-x)*x^x", "x/x/x/x", "x/x-x"];
            println!("Starting formula parsing tests");
            inputs.into_iter().for_each(test_formula_parsing);
        }

        fn test_formula_parsing(input: &str) {
            let cow = Element::preprocess_string_minus(&input);

            println!();
            print_heading("Starting formula parsing");
            println!();

            println!("Input: {}", cow);
            let chars = cow.chars().collect::<Vec<_>>();

            print_heading("0. Bracketize and process operations");

            let mut start = 0;
            let mut brackets = Element::bracketize(&chars, &mut start);

            brackets.print();

            debug_print_step("1. Processing '+'", &mut brackets, Element::process_plus);
            debug_print_step("2. Processing '-'", &mut brackets, Element::process_minus);
            debug_print_step("3. Processing '*'", &mut brackets, Element::process_multiply);
            debug_print_step("4. Processing '/'", &mut brackets, Element::process_divide);
            debug_print_step("5. Processing '-' again", &mut brackets, Element::process_minus);
            debug_print_step("6. Processing '^'", &mut brackets, Element::process_pow);
            debug_print_step("7. Processing '-' again", &mut brackets, Element::process_minus);
            debug_print_step(
                "8. Convert to numbers and variables",
                &mut brackets,
                Element::process_numbers_and_variables,
            );
        }

        fn debug_print_step(step: &str, element: &mut Element, operation: fn(&mut Element)) {
            print_heading(step);
            operation(element);
            element.print();
        }

        fn print_heading(step: &str) {
            println!("##### {}", step);
        }
    }
}

mod evaluation {
    use crate::parsing::implementation::Element;

    impl Element {
        pub fn eval(&self) -> Option<f64> {
            match self {
                Element::Brackets(_) | Element::String(_) | Element::Variable(_) => None,
                Element::Plus(elements) => {
                    let mut sum = 0.0;
                    for n in elements {
                        sum += n.eval()?;
                    }
                    Some(sum)
                },
                Element::Multiply(elements) => {
                    let mut product = 1.0;
                    for n in elements {
                        product *= n.eval()?;
                    }
                    Some(product)
                },
                Element::Negate(e) => e.eval().map(|n| -n),
                Element::Number(n) => Some(*n),
                Element::Pow(b, e) => Some(b.eval()?.powf(e.eval()?)),
            }
        }
    }
}
