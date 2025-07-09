use crate::parsing::testing::test_with_user_input;

fn main() {
    test_with_user_input();
}

#[derive(Debug, Clone)]
pub enum Element {
    /// Unparsed group of elements
    Brackets(Vec<Element>),
    /// Unparsed string
    String(String),

    /// List of elements to add together
    Plus(Vec<Element>),
    /// List of elements to multiply together
    Multiply(Vec<Element>),
    /// Exponential operation (base^exponent)
    Pow(Box<Element>, Box<Element>),
    /// Negation of an element (e.g., -x)
    Negate(Box<Element>),

    /// A number
    Number(f64),
    /// A function with a name and arguments
    Function { name: String, arguments: Vec<Element> },
    /// A variable with a name
    Variable(String),
    /// A variable, which could either be a number or a function
    VariableOrFunction(String),
}

mod parsing {
    pub mod implementation {
        use crate::Element;
        use regex::Regex;
        use std::mem;

        fn is_valid_char_for_function_name(c: char) -> bool {
            matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9')
        }
        fn get_name_of_function(input: &[char], start: usize) -> Option<String> {
            if start < 2 {
                return None;
            }
            let end_exclusive = start - 1;
            let mut start = end_exclusive;
            while start > 0 && is_valid_char_for_function_name(input[start - 1]) {
                start -= 1;
            }
            if start == end_exclusive {
                return None;
            }
            let name = input[start..end_exclusive].iter().collect::<String>();
            if matches!(name.chars().nth(0), Some('0'..='9')) {
                return None; // Function names cannot start with a digit
            }
            Some(name)
        }

        fn get_largest_fun_name(name: &str) -> String {
            let mut valid_chars_count = 0;
            name.chars()
                .rev()
                .take_while(|&c| is_valid_char_for_function_name(c))
                .for_each(|_| valid_chars_count += 1);
            if valid_chars_count == 0 {
                "".to_owned()
            } else {
                name[name.len() - valid_chars_count..].to_owned()
            }
        }

        impl Element {
            pub fn parse(input: &str) -> Option<Self> {
                let cow = Element::preprocess_string_minus(&input);
                let chars = cow.chars().collect::<Vec<_>>();
                let mut start = 0;
                let mut formula = Element::resolve_brackets(&chars, &mut start);
                formula.resolve_functions();
                formula.process_plus();
                formula.process_minus();
                formula.process_multiply();
                formula.process_divide();
                formula.process_minus();
                formula.process_pow();
                formula.process_minus();
                formula.process_numbers_and_variables();
                formula.remove_unneeded_outer_brackets();
                if formula.anything_unparsed() { None } else { Some(formula) }
            }

            /// Step 0
            pub(super) fn preprocess_string_minus(input: &str) -> String {
                let without_whitespace = input.replace(" ", "");
                Regex::new(r"([\w)])-([\w(])")
                    .unwrap()
                    .replace_all(&without_whitespace, "$1+-$2")
                    .to_string()
            }

            /// Step 1
            pub(super) fn resolve_brackets(input: &[char], start: &mut usize) -> Element {
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
                        elements.push(Self::resolve_brackets(&input, &mut i));
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

            /// Step 0,5
            pub(super) fn resolve_functions(&mut self) {
                match self {
                    Element::Brackets(elements) => {
                        for i in (0..elements.len() - 1).rev() {
                            let j = i + 1;
                            if let (Element::String(name), Element::Brackets(br_elements)) =
                                (&elements[i], &elements[j])
                            {
                                let name = name.to_string();
                                let function_name = get_largest_fun_name(&name);
                                if function_name
                                    .chars()
                                    .nth(0)
                                    .is_none_or(|c| matches!(c, '0'..='9'))
                                {
                                    continue; // Function names cannot be empty or start with a digit
                                }

                                // create the new function element
                                let arguments = split_list_by_char(br_elements, ',')
                                    .unwrap_or_else(|| {
                                        vec![Element::Brackets(br_elements.clone())]
                                    });

                                elements[j] =
                                    Element::Function { name: function_name.clone(), arguments };

                                // update or remove the string element
                                let new_str_len = name.len() - function_name.len();
                                if new_str_len == 0 {
                                    elements.remove(i);
                                } else {
                                    elements[i] = Element::String(name[..new_str_len].to_owned());
                                }
                            }
                        }
                    },
                    Element::Plus(elements) => {
                        elements.iter_mut().for_each(Element::resolve_functions);
                    },
                    Element::Multiply(elements) => {
                        elements.iter_mut().for_each(Element::resolve_functions);
                    },
                    Element::Negate(element) => element.resolve_functions(),
                    Element::Pow(base, exponent) => {
                        base.resolve_functions();
                        exponent.resolve_functions();
                    },
                    _ => {},
                }
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
                    Element::Function { arguments, .. } => {
                        arguments.iter_mut().for_each(Element::process_plus);
                    },
                    Element::String(s) => {
                        if let Some(elements) = split_string_by_char(s, '+') {
                            *self = Element::Plus(elements);
                        }
                    },
                    Element::Plus(_)
                    | Element::Multiply(_)
                    | Element::Negate(_)
                    | Element::Variable(_)
                    | Element::VariableOrFunction(_)
                    | Element::Number(_)
                    | Element::Pow(_, _) => {},
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
                    Element::Function { arguments, .. } => {
                        arguments.iter_mut().for_each(Element::process_minus)
                    },
                    Element::Variable(_) | Element::Number(_) | Element::VariableOrFunction(_) => {
                    },
                }
            }

            /// Step 3
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
                    Element::Function { arguments, .. } => {
                        arguments.iter_mut().for_each(Element::process_multiply)
                    },
                    Element::Multiply(_)
                    | Element::Variable(_)
                    | Element::Number(_)
                    | Element::VariableOrFunction(_)
                    | Element::Pow(_, _) => {},
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
                    Element::Function { arguments, .. } => {
                        arguments.iter_mut().for_each(Element::process_divide);
                    },
                    Element::Negate(_)
                    | Element::Variable(_)
                    | Element::Number(_)
                    | Element::VariableOrFunction(_)
                    | Element::Pow(_, _) => {},
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
                    Element::Function { arguments, .. } => {
                        arguments.iter_mut().for_each(Element::process_pow);
                    },
                    Element::Variable(_) | Element::Number(_) | Element::VariableOrFunction(_) => {
                    },
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
                    Element::Function { arguments, .. } => {
                        arguments.iter_mut().for_each(|e| {
                            if let Element::String(s) = e {
                                if let Ok(num) = s.parse::<f64>() {
                                    *e = Element::Number(num);
                                } else {
                                    *e = Element::VariableOrFunction(s.clone());
                                }
                            } else {
                                e.process_numbers_and_variables();
                            }
                        });
                    },
                    Element::Variable(_) | Element::Number(_) | Element::VariableOrFunction(_) => {
                    },
                }
            }

            /// Step 9
            pub(super) fn remove_unneeded_outer_brackets(&mut self) {
                match self {
                    Element::Brackets(elements) => {
                        elements.iter_mut().for_each(Element::remove_unneeded_outer_brackets);
                        if elements.len() == 1 {
                            *self = elements.remove(0);
                        }
                    },
                    Element::Plus(elements)
                    | Element::Multiply(elements)
                    | Element::Function { arguments: elements, .. } => {
                        elements.iter_mut().for_each(Element::remove_unneeded_outer_brackets)
                    },
                    Element::Pow(base, exponent) => {
                        base.remove_unneeded_outer_brackets();
                        exponent.remove_unneeded_outer_brackets();
                    },
                    Element::Negate(element) => element.remove_unneeded_outer_brackets(),
                    Element::Variable(_)
                    | Element::Number(_)
                    | Element::String(_)
                    | Element::VariableOrFunction(_) => {},
                }
            }

            fn anything_unparsed(&self) -> bool {
                match self {
                    Element::Brackets(_) | Element::String(_) => true,
                    Element::Plus(elements)
                    | Element::Multiply(elements)
                    | Element::Function { arguments: elements, .. } => {
                        elements.iter().any(Element::anything_unparsed)
                    },
                    Element::Pow(base, exponent) => {
                        base.anything_unparsed() || exponent.anything_unparsed()
                    },
                    Element::Negate(element) => element.anything_unparsed(),
                    Element::Variable(_) | Element::Number(_) | Element::VariableOrFunction(_) => {
                        false
                    },
                }
            }
        }

        fn list_contains_char(input: &[Element], delimiter: char) -> bool {
            input
                .iter()
                .any(|e| if let Element::String(s) = e { s.contains(delimiter) } else { false })
        }

        fn split_list_by_char(input: &[Element], delimiter: char) -> Option<Vec<Element>> {
            const DEBUG: bool = false;
            if DEBUG {
                println!("Processing list of elements: {:?}", input);
            }
            if !list_contains_char(input, delimiter) {
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
        use crate::Element;
        use std::io::Write;

        pub fn test_with_user_input() {
            use std::io::stdin;

            loop {
                print!("Enter a formula to parse (or '' to quit):\n> ");
                std::io::stdout().flush().unwrap();
                let mut input = String::new();
                stdin().read_line(&mut input).unwrap();
                let input = input.trim();

                if input.is_empty() {
                    return;
                }

                let Some(element) = Element::parse(input) else {
                    println!("Could not parse the formula: {}", input);
                    println!();
                    continue;
                };
                println!();
                println!("Parsed formula:");
                element.print();
                print!("= ");
                element.print_debug();
                println!();
                if let Some(num) = element.eval() {
                    println!("Calculated Result: {}", num);
                } else {
                    println!("Calculated Result: Could not evaluate the formula.");
                }
                println!();
            }
        }

        pub fn run_tests() {
            let inputs = [
                "((x/x-x)*-x^x)/(x-x)^-x",
                "(x/x+-x)*x^x",
                "x/x/x/x",
                "x/x-x",
                "123",
                "x",
                "1+((2))",
                "a,b,c",
                "a(a,c)",
                "a(a+c)",
                "m+a(a,b+c)",
            ];
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
            let mut brackets = Element::resolve_brackets(&chars, &mut start);

            brackets.print_debug();

            debug_print_step("0,5. Resolve functions", &mut brackets, Element::resolve_functions);
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
            debug_print_step(
                "9. Removing unneeded outer brackets",
                &mut brackets,
                Element::remove_unneeded_outer_brackets,
            );
        }

        fn debug_print_step(step: &str, element: &mut Element, operation: fn(&mut Element)) {
            print_heading(step);
            operation(element);
            element.print_debug();
        }

        fn print_heading(step: &str) {
            println!("##### {}", step);
        }
    }
}

mod printing {
    use crate::Element;
    use colored::Colorize;

    enum Inner<'a, T: 'a>
    where
        T: IntoIterator<Item = &'a Element>,
    {
        Single(&'a Element),
        Multiple { delimiter: &'a str, elements: T },
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
                    Inner::Multiple { delimiter: "", elements },
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
                Element::String(s) => add_element_string(show_types, "str", s, output),
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
}

mod evaluation {
    use crate::Element;

    impl Element {
        pub fn eval(&self) -> Option<f64> {
            match self {
                Element::Brackets(_)
                | Element::String(_)
                | Element::Variable(_)
                | Element::Function { .. }
                | Element::VariableOrFunction(_) => None,

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
