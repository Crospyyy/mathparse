use colored::Colorize;
use regex::Regex;
use std::borrow::Cow;

fn main() {
    let inputs = ["((x/x-x)*x^x)/(x-x)^-x", "x/x/x/x", "x/x-x"];
    inputs.into_iter().for_each(test_formula_parsing)
}

fn test_formula_parsing(input: &str) {
    let cow = Element::preprocess_string_minus(&input);
    println!("{}", cow);
    let chars = cow.chars().collect::<Vec<_>>();

    println!("{}", "### Bracketize and process operations ###".yellow());

    let mut start = 0;
    let mut brackets = Element::bracketize(&chars, &mut start);

    println!("{:?}", brackets);
    brackets.debug_print(0, true);
    println!();

    debug_print_step("Processing '+'", &mut brackets, Element::process_plus);
    debug_print_step("Processing '-'", &mut brackets, Element::process_minus);
    debug_print_step("Processing '*'", &mut brackets, Element::process_multiply);
    debug_print_step("Processing '/'", &mut brackets, Element::process_divide);
}

fn debug_print_step(step: &str, element: &mut Element, operation: fn(&mut Element)) {
    println!("{}", format!("### {} ###", step).yellow());
    operation(element);
    println!("{:?}", element);
    element.debug_print(0, true);
    println!();
}

#[derive(Debug, Clone)]
enum Element {
    Brackets(Vec<Element>),
    Plus(Vec<Element>),
    Multiply(Vec<Element>),
    Negate(Box<Element>),
    String(String),
    Pow(Box<Element>, Box<Element>),
}

impl Element {
    fn preprocess_string_minus(input: &str) -> Cow<str> {
        Regex::new(r"([x)])-([x(])").unwrap().replace_all(input, "$1+-$2")
    }
    /// Step 1
    fn bracketize(input: &[char], start: &mut usize) -> Element {
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
    fn process_plus(&mut self) {
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

    /// Step 3
    fn process_minus(&mut self) {
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
            },
            Element::Plus(elements) => {
                for element in elements {
                    element.process_minus();
                }
            },
            Element::String(s) => {
                if s.starts_with('-') {
                    *self = Element::Negate(Box::new(Element::String(s[1..].to_owned())));
                }
            },
            Element::Negate(element) => {
                element.process_minus();
            },
            _ => {},
        }
    }

    /// Step 4
    fn process_multiply(&mut self) {
        match self {
            Element::Brackets(elements) => {
                if let Some(groups) = split_list_by_char(elements, '*') {
                    *self = Element::Multiply(groups);
                }
                if let Element::Multiply(elements) = self {
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
            _ => {},
        }
    }

    fn invert(&mut self) {
        *self = Element::Pow(
            Box::new(self.clone()),
            Box::new(Element::Negate(Box::new(Element::String("1".to_string())))),
        );
    }

    /// Step 4 in between
    fn process_divide(&mut self) {
        match self {
            Element::String(str) => {
                if let Some(mut elements) = split_string_by_char(str, '/') {
                    elements[1..].iter_mut().for_each(Element::invert);
                    *self = Element::Multiply(elements);
                }
            },
            Element::Brackets(elements) => {
                if let Some(mut new_elements) = split_list_by_char(elements, '/') {
                    new_elements[1..].iter_mut().for_each(Element::invert);
                    *self = Element::Multiply(new_elements);
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

    fn debug_print(&self, indent: usize, one_line: bool) {
        match self {
            Element::Brackets(elements) => {
                Self::print_indented(indent, "(", one_line, true);
                for element in elements {
                    element.debug_print(indent + 1, one_line);
                }
                Self::print_indented(indent, ")", one_line, true);
            },
            Element::Plus(elements) => {
                Self::print_indented(indent, "(", one_line, true);
                for (i, element) in elements.iter().enumerate() {
                    element.debug_print(indent + 1, one_line);
                    if i + 1 < elements.len() {
                        Self::print_indented(indent + 1, "+", one_line, true);
                    }
                }
                Self::print_indented(indent, ")", one_line, true);
            },
            Element::Multiply(elements) => {
                Self::print_indented(indent, "(", one_line, true);
                for (i, element) in elements.iter().enumerate() {
                    element.debug_print(indent + 1, one_line);
                    if i + 1 < elements.len() {
                        Self::print_indented(indent + 1, "*", one_line, true);
                    }
                }
                Self::print_indented(indent, ")", one_line, true);
            },
            Element::Negate(element) => {
                Self::print_indented(indent, "-", one_line, true);
                element.debug_print(indent + 1, one_line);
            },
            Element::String(s) => Self::print_indented(indent, s, one_line, false),
            Element::Pow(base, exponent) => {
                base.debug_print(indent + 1, one_line);
                Self::print_indented(indent + 1, "^", one_line, true);
                exponent.debug_print(indent + 1, one_line);
            },
        }
    }

    fn print_indented(indent: usize, str: &str, same_line: bool, color: bool) {
        let str = if color { str.green() } else { str.normal() };
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
                    println!("String contains '{delimiter}', splitting into parts: {:?}", parts);
                }
                for (i, part) in parts.iter().enumerate() {
                    if i == 0 {
                        if part.is_empty() {
                            if !current_group.is_empty() {
                                groups.push(Element::Brackets(current_group));
                                current_group = Vec::new();
                            }
                        } else {
                            current_group.push(Element::String(part.to_string()));
                        }
                    } else if i > 0 {
                        groups.push(Element::Brackets(current_group));
                        current_group = Vec::new();
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

    if !current_group.is_empty() {
        groups.push(Element::Brackets(current_group));
    }

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
