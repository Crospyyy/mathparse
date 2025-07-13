pub mod implementation {
    use crate::Element;
    use regex::Regex;
    use std::mem;

    fn is_valid_char_for_function_name(c: char) -> bool {
        matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9')
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
                            if function_name.chars().nth(0).is_none_or(|c| matches!(c, '0'..='9')) {
                                continue; // Function names cannot be empty or start with a digit
                            }

                            // create the new function element
                            let arguments = split_list_by_char(br_elements, ',')
                                .unwrap_or_else(|| vec![Element::Brackets(br_elements.clone())]);

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
                Element::Variable(_) | Element::Number(_) | Element::VariableOrFunction(_) => {},
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
                Element::Plus(elements) => elements.iter_mut().for_each(Element::process_multiply),
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
            let create_recursive_pow = |element: &mut Element, mut new_elements: Vec<Element>| {
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
                Element::Variable(_) | Element::Number(_) | Element::VariableOrFunction(_) => {},
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
                Element::Variable(_) | Element::Number(_) | Element::VariableOrFunction(_) => {},
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
                Element::Variable(_) | Element::Number(_) | Element::VariableOrFunction(_) => false,
            }
        }
    }

    fn list_contains_char(input: &[Element], delimiter: char) -> bool {
        input.iter().any(|e| if let Element::String(s) = e { s.contains(delimiter) } else { false })
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
                        println!("String does not contain '{delimiter}', adding to current group.");
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

    #[allow(unused)]
    pub fn test_with_user_input() {
        use std::io::Write;
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
            println!("Parsed formula");
            print!("= ");
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

    #[test]
    pub fn run_all_tests() {
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

        brackets.print();
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
        assert_eq!(brackets, Element::parse(input).unwrap());
    }

    fn debug_print_step(step: &str, element: &mut Element, operation: fn(&mut Element)) {
        print_heading(step);
        operation(element);
        element.print();
        element.print_debug();
    }

    fn print_heading(step: &str) {
        println!("##### {}", step);
    }
}

pub mod signature {
    use crate::Element;
    use std::collections::{HashMap, HashSet};
    use std::ops::Deref;

    #[derive(Debug, Clone)]
    pub enum Signature {
        NumberOrFunction,
        Function(Vec<Signature>),
        Number,
        Conflicting,
    }

    impl Signature {
        fn refine_with(&mut self, new: Self) {
            if matches!(self, Signature::Conflicting) {
                return;
            }
            if matches!(&new, Signature::Conflicting) {
                *self = new;
                return;
            }
            if new.get_priority() > self.get_priority() {
                *self = new;
                return;
            }
            if new.get_priority() < self.get_priority() {
                return;
            }
            match (&mut *self, new) {
                (Signature::Number, Signature::Number) => {},
                (Signature::Function(_), Signature::Number)
                | (Signature::Number, Signature::Function(_)) => *self = Signature::Conflicting,
                (Signature::Function(args_old), Signature::Function(args_new)) => {
                    if args_old.len() != args_new.len() {
                        *self = Signature::Conflicting;
                        return;
                    }
                    args_old.iter_mut().zip(args_new).for_each(|(a, b)| {
                        a.refine_with(b);
                    });
                },
                (_, _) => {},
            }
        }

        fn get_priority(&self) -> u8 {
            match self {
                Signature::NumberOrFunction => 0,
                Signature::Number => 1,
                Signature::Function(_) => 1,
                Signature::Conflicting => 1,
            }
        }

        fn could_be(&self, other: &Signature) -> bool {
            match (self, other) {
                (Signature::NumberOrFunction, _) => {
                    if !matches!(other, Signature::Conflicting) {
                        true
                    } else {
                        false
                    }
                },
                (_, Signature::NumberOrFunction) => false,
                (Signature::Function(args_0), Signature::Function(args_1)) => {
                    if args_0.len() != args_1.len() {
                        return false;
                    }
                    args_0.iter().zip(args_1).all(|(a, b)| a.could_be(b))
                },
                (Signature::Number, Signature::Number) => true,
                _ => false,
            }
        }
    }

    #[derive(Clone)]
    pub struct Signatures(HashMap<String, Signature>);

    impl Signatures {
        fn new_empty() -> Self {
            Signatures(HashMap::new())
        }

        pub fn generate_needed_elements_of_formula(formula: &Element) -> Signatures {
            let mut all_undefined = Signatures::new_empty();
            all_undefined.add_all_undefined_symbols_of_formula(formula);
            all_undefined
        }

        fn add_all_undefined_symbols_of_formula(&mut self, element: &Element) {
            match element {
                Element::Brackets(elements)
                | Element::Plus(elements)
                | Element::Multiply(elements) => {
                    elements.iter().for_each(|e| self.add_all_undefined_symbols_of_formula(e))
                },
                Element::Pow(base, exponent) => {
                    self.add_all_undefined_symbols_of_formula(base);
                    self.add_all_undefined_symbols_of_formula(exponent);
                },
                Element::Negate(element) => {
                    self.add_all_undefined_symbols_of_formula(element);
                },
                Element::Function { name, arguments } => {
                    arguments.iter().for_each(|a| self.add_all_undefined_symbols_of_formula(a));

                    let arg_signatures = arguments
                        .iter()
                        .map(|arg| match arg {
                            Element::Number(_) => Signature::Number,
                            Element::VariableOrFunction(_) => Signature::NumberOrFunction,
                            _ => panic!("Invalid element in function arguments: {:?}", arg),
                        })
                        .collect::<Vec<_>>();

                    self.insert_or_replace_symbol(name, Signature::Function(arg_signatures));
                },
                Element::Variable(name) => self.insert_or_replace_symbol(name, Signature::Number),
                Element::VariableOrFunction(name) => {
                    self.insert_or_replace_symbol(name, Signature::NumberOrFunction)
                },
                Element::Number(_) => {},
                Element::String(_) => {},
            }
        }

        fn insert_or_replace_symbol(&mut self, name: &String, val: Signature) {
            if let Some(entry) = self.0.get_mut(name) {
                entry.refine_with(val)
            } else {
                self.0.insert(name.clone(), val);
            }
        }

        fn add_symbol_from_function_signature_and_definition(
            &mut self, name_and_args: Element, content: Element,
        ) -> Result<(), String> {
            let mut symbol_name_and_args = SymbolDeclarationData::from_formula(&name_and_args)?;

            if self.0.get(&symbol_name_and_args.name).is_some() {
                return Err(format!(
                    "The formula {} is already defined",
                    symbol_name_and_args.name
                ));
            }

            let mut required_signatures = Signatures::generate_needed_elements_of_formula(&content);

            Self::refine_signature_and_undefined(
                &mut symbol_name_and_args,
                &mut required_signatures,
                &content,
                self,
            );

            if !required_signatures.0.is_empty() {
                return Err(format!(
                    "The formula {} requires the following elements to be defined: {:?}",
                    symbol_name_and_args.name, required_signatures.0
                ));
            }

            if let Some(args) = symbol_name_and_args.function_args {
                self.0.insert(
                    symbol_name_and_args.name,
                    Signature::Function(args.get_signatures_in_right_order()),
                );
            } else {
                self.0.insert(symbol_name_and_args.name, Signature::Number);
            }

            Ok(())
        }

        fn refine_signature_and_undefined(
            symbol_name_and_args: &mut SymbolDeclarationData,
            undefined_signatures: &mut Signatures, formula: &Element, already_defined: &Signatures,
        ) {
            let parameter_names = symbol_name_and_args
                .function_args
                .as_ref()
                .map(|v| {
                    let mut set = HashSet::new();
                    for e in &v.names {
                        set.insert(e.clone());
                    }
                    set
                })
                .unwrap_or(HashSet::new());

            let all_undefined_names =
                undefined_signatures.0.iter().map(|(n, _)| n).cloned().collect::<HashSet<_>>();
            for name in all_undefined_names {
                if parameter_names.contains(&name) {
                    continue;
                }
                if let Some(sig) = already_defined.0.get(&name) {
                    undefined_signatures.update_signature(&formula, &name, sig.clone())
                }
            }
            if let Some(args) = &mut symbol_name_and_args.function_args {
                for (param_name, param_sig) in &mut args.signatures.0 {
                    param_sig.refine_with(undefined_signatures.0[param_name].clone())
                }
            }
            undefined_signatures.0.retain(|n, _| !parameter_names.contains(n));
            undefined_signatures.0.retain(|n, _| !already_defined.0.contains_key(n));
        }

        fn add_symbol_from_string(&mut self, string: &str) -> Result<(), String> {
            let (sig, def) =
                string.split_once("=").ok_or("String doesn't contain '='".to_owned())?;
            let sig = Element::parse(sig).ok_or("First formula could not be parsed")?;
            let def = Element::parse(def).ok_or("Second formula could not be parsed")?;
            self.add_symbol_from_function_signature_and_definition(sig, def)
        }

        fn update_signature(
            &mut self, formula: &Element, element_to_update: &str, new_signature: Signature,
        ) {
            let Some(signature) = self.0.get_mut(element_to_update) else { return };
            signature.refine_with(new_signature.clone());

            // update all functions that contain this symbol as a parameter
            let mut list_all_parameter_occurrences = HashSet::new();
            formula.list_all_functions_with_argument_variable(
                element_to_update,
                &mut list_all_parameter_occurrences,
            );
            for (fn_name, param_index) in list_all_parameter_occurrences {
                let mut new_fn_signature = self.0[&fn_name].clone();
                match &mut new_fn_signature {
                    Signature::Function(args) => args[param_index] = new_signature.clone(),
                    _ => panic!("This shouldn't happen"),
                }
                self.update_signature(formula, &fn_name, new_fn_signature);
            }

            if let Signature::Function(args) = new_signature {
                // update all parameters, of which their types may be affected
                let mut all_params_of_function_type = HashSet::new();
                formula.list_all_function_arguments_where_function_has_name(
                    element_to_update,
                    &mut all_params_of_function_type,
                );
                for (param_name, param_index) in all_params_of_function_type {
                    let new_arg_signature = args[param_index].clone();
                    self.0.get_mut(&param_name).unwrap().refine_with(new_arg_signature);
                }
            }
        }
    }

    impl Element {
        fn get_name(&self) -> Option<&str> {
            match self {
                Element::Function { name, .. }
                | Element::Variable(name)
                | Element::VariableOrFunction(name) => Some(name),
                _ => None,
            }
        }

        fn name_matches(&self, name: &str) -> bool {
            match self {
                Element::Function { name: name_cmp, .. }
                | Element::Variable(name_cmp)
                | Element::VariableOrFunction(name_cmp) => name_cmp == name,
                _ => false,
            }
        }

        /// Returns a set of Function names and indices, which parameter is equal to the ```name```
        fn list_all_functions_with_argument_variable(
            &self, name: &str, list: &mut HashSet<(String, usize)>,
        ) {
            match self {
                Element::Function { arguments, name: this_name } => {
                    for (i, _) in arguments.iter().enumerate().filter(|(_, e)| e.name_matches(name))
                    {
                        list.insert((this_name.clone(), i));
                    }
                    arguments
                        .iter()
                        .for_each(|a| a.list_all_functions_with_argument_variable(name, list))
                },
                Element::Plus(elements) | Element::Multiply(elements) => elements
                    .iter()
                    .for_each(|a| a.list_all_functions_with_argument_variable(name, list)),
                Element::Pow(a, b) => {
                    a.list_all_functions_with_argument_variable(name, list);
                    b.list_all_functions_with_argument_variable(name, list);
                },
                Element::Negate(e) => e.list_all_functions_with_argument_variable(name, list),
                Element::Brackets(_)
                | Element::String(_)
                | Element::Number(_)
                | Element::Variable(_)
                | Element::VariableOrFunction(_) => {},
            }
        }

        /// Returns a set of Function names and indices, which parameter is equal to the ```name```
        fn list_all_function_arguments_where_function_has_name(
            &self, name: &str, list: &mut HashSet<(String, usize)>,
        ) {
            match self {
                Element::Function { arguments, name: this_name } => {
                    if this_name == name {
                        for (i, arg_name) in arguments
                            .iter()
                            .enumerate()
                            .map(|(i, e)| e.get_name().map(|n| (i, n)))
                            .flatten()
                        {
                            if arg_name != name {
                                list.insert((arg_name.to_string(), i));
                            }
                        }
                    }
                    arguments
                        .iter()
                        .for_each(|a| a.list_all_functions_with_argument_variable(name, list))
                },
                Element::Plus(elements) | Element::Multiply(elements) => elements
                    .iter()
                    .for_each(|a| a.list_all_functions_with_argument_variable(name, list)),
                Element::Pow(a, b) => {
                    a.list_all_functions_with_argument_variable(name, list);
                    b.list_all_functions_with_argument_variable(name, list);
                },
                Element::Negate(e) => e.list_all_functions_with_argument_variable(name, list),
                Element::Brackets(_)
                | Element::String(_)
                | Element::Number(_)
                | Element::Variable(_)
                | Element::VariableOrFunction(_) => {},
            }
        }
    }

    struct SymbolDeclarationData {
        name: String,
        function_args: Option<FunctionDeclarationArguments>,
    }

    struct FunctionDeclarationArguments {
        names: Vec<String>,
        signatures: Signatures,
    }

    impl FunctionDeclarationArguments {
        fn get_signatures_in_right_order(&self) -> Vec<Signature> {
            self.names
                .iter()
                .map(|name| self.signatures.0.get(name).unwrap().clone())
                .collect::<Vec<_>>()
        }
    }

    impl SymbolDeclarationData {
        fn from_formula(formula: &Element) -> Result<Self, String> {
            let insert_name;
            let function_args;
            match formula {
                Element::Variable(name) => {
                    function_args = None;
                    insert_name = name;
                },
                Element::Function { name, arguments } => {
                    insert_name = name;
                    let mut fn_args = FunctionDeclarationArguments {
                        names: Vec::new(),
                        signatures: Signatures::new_empty(),
                    };
                    for arg in arguments {
                        if let Element::VariableOrFunction(name) = arg {
                            fn_args.names.push(name.clone());
                            fn_args.signatures.0.insert(name.clone(), Signature::NumberOrFunction);
                        } else {
                            return Err("Invalid argument in function signature".to_string());
                        }
                    }
                    function_args = Some(fn_args)
                },
                _ => {
                    return Err("Invalid formula signature provided".to_string());
                },
            }
            Ok(Self { name: insert_name.to_owned(), function_args })
        }
    }

    fn update_signature(
        mut signature_arguments: Option<&mut Signatures>, undefined: &mut Signatures,
        refine_with: &Signatures,
    ) -> Result<(), String> {
        let mut result = Ok(());
        undefined.0.retain(|name, sig| {
            if let Some(fun_arg) = signature_arguments.as_mut().and_then(|a| a.0.get_mut(name)) {
                if fun_arg.could_be(&sig) {
                    fun_arg.refine_with(sig.clone());
                    false
                } else {
                    result = Err(format!("Invalid usage of already defined formula {}", name));
                    true
                }
            } else if let Some(already_defined) = refine_with.0.get(name) {
                if sig.could_be(already_defined) {
                    false
                } else {
                    result = Err(format!("Invalid usage of already defined formula {}", name));
                    true
                }
            } else {
                true
            }
        });
        result
    }

    struct SymbolDefinition {}

    #[test]
    fn test_symbols() {
        let mut all = Signatures::new_empty();
        assert_eq!(all.add_symbol_from_string("fun(a,b)=a+b"), Ok(()));
        assert!(matches!(all.add_symbol_from_string("fun(a,b)=a+b"), Err(_)));
        println!("{:?}", all.0);
        assert!(matches!(all.add_symbol_from_string("fun2(a,b,c)=fun(a,b)+c"), Ok(())));
        println!("{:?}", all.0);
    }
}
