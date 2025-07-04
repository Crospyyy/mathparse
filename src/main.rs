use colored::Colorize;
use regex::Regex;

fn main() {
    let mut input = "((x/x-x)*x^x)/(x-x)^-x".to_owned();
    let cow = Regex::new(r"([x)])-([x(])").unwrap().replace_all(&mut input, "$1+-$2");
    println!("{}", cow);
    let chars = cow.chars().collect::<Vec<_>>();

    println!("{}", "### Bracketize and process operations ###".yellow());

    let mut start = 0;
    let mut brackets = Element::bracketize(&chars, &mut start);

    println!("{:?}", brackets);
    brackets.debug_print(0, true);
    println!();

    println!("{}", "### Process '+' ###".yellow());

    brackets.process_plus();

    println!("{:?}", brackets);
    brackets.debug_print(0, true);
    println!();

    println!("{}", "### Process '-' ###".yellow());

    brackets.process_minus();

    println!("{:?}", brackets);
    brackets.debug_print(0, true);
    println!();

    println!("{}", "### Process '*' ###".yellow());

    brackets.process_multiply();

    println!("{:?}", brackets);
    brackets.debug_print(0, true);
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
                println!("Processing brackets: {:?}", elements);
                let any_plus = elements
                    .iter()
                    .any(|e| if let Element::String(s) = e { s.contains('+') } else { false });
                if any_plus {
                    println!("Found '+' in brackets, processing...");
                    let mut groups = Vec::new();
                    let mut current_group = Vec::new();
                    for element in elements {
                        if let Element::String(str) = element {
                            println!("Processing string element: {:?}", str);
                            if !str.contains('+') {
                                println!("String does not contain '+', adding to current group.");
                                current_group.push(element.clone());
                            } else {
                                let parts: Vec<&str> = str.split('+').collect();
                                println!("String contains '+', splitting into parts: {:?}", parts);
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
                            println!("Processing non-string element: {:?}", element);
                            current_group.push(element.clone());
                        }
                    }

                    if !current_group.is_empty() {
                        groups.push(Element::Brackets(current_group));
                    }

                    *self = Element::Plus(groups);
                } else {
                    println!("No '+' found in brackets, processing elements directly.");
                }
                match self {
                    Element::Brackets(elements) | Element::Plus(elements) => {
                        elements.iter_mut().for_each(Element::process_plus);
                    },
                    _ => {},
                }
            },
            Element::String(s) => {
                if s.contains('+') {
                    let parts: Vec<&str> = s.split('+').collect();
                    let mut new_elements = Vec::new();
                    for part in parts {
                        if !part.is_empty() {
                            new_elements.push(Element::String(part.to_string()));
                        }
                    }
                    *self = Element::Plus(new_elements);
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
                println!("Processing brackets: {:?}", elements);
                let any_multiply = elements
                    .iter()
                    .any(|e| if let Element::String(s) = e { s.contains('*') } else { false });
                if any_multiply {
                    println!("Found '*' in brackets, processing...");
                    let mut groups = Vec::new();
                    let mut current_group = Vec::new();
                    for element in elements {
                        if let Element::String(str) = element {
                            println!("Processing string element: {:?}", str);
                            if !str.contains('*') {
                                println!("String does not contain '*', adding to current group.");
                                current_group.push(element.clone());
                            } else {
                                let parts: Vec<&str> = str.split('*').collect();
                                println!("String contains '*', splitting into parts: {:?}", parts);
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
                            println!("Processing non-string element: {:?}", element);
                            current_group.push(element.clone());
                        }
                    }

                    if !current_group.is_empty() {
                        groups.push(Element::Brackets(current_group));
                    }

                    *self = Element::Multiply(groups);
                } else {
                    println!("No '*' found in brackets, processing elements directly.");
                }
                match self {
                    Element::Multiply(elements) => {
                        elements.iter_mut().for_each(Element::process_multiply);
                    },
                    _ => {},
                }
            },
            Element::Plus(elements) => {
                for element in elements {
                    element.process_multiply();
                }
            },
            Element::Negate(element) => {
                element.process_multiply();
            },
            Element::String(s) => {
                if s.contains('*') {
                    let parts: Vec<&str> = s.split('*').collect();
                    let mut new_elements = Vec::new();
                    for part in parts {
                        if !part.is_empty() {
                            new_elements.push(Element::String(part.to_string()));
                        }
                    }
                    *self = Element::Multiply(new_elements);
                    self.process_multiply();
                }
            },
            _ => {},
        }
    }

    /// Step 4 in between
    fn process_divide_non_rec(&mut self) {
        match self {
            Element::String(str) => {
                if str.contains('/') {
                    let parts: Vec<&str> = str.split('/').collect();
                    let mut new_elements = Vec::new();
                    for part in parts {
                        if !part.is_empty() {
                            new_elements.push(Element::String(part.to_string()));
                        }
                    }
                    for e in &mut new_elements[1..] {
                        *e = Element::Pow(
                            Box::new(e.clone()),
                            Box::new(Element::String("-1".to_string())),
                        );
                    }
                    *self = Element::Multiply(new_elements);
                }
            },
            Element::Brackets(elements) => {
                todo!("Implement divide processing for brackets");
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
                for element in elements {
                    element.debug_print(indent + 1, one_line);
                    Self::print_indented(indent + 1, "*", one_line, true);
                }
                Self::print_indented(indent, ")", one_line, true);
            },
            Element::Negate(element) => {
                Self::print_indented(indent, "-(", one_line, true);
                element.debug_print(indent + 1, one_line);
                Self::print_indented(indent, ")", one_line, true);
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

fn split_by_char(input: &[Element], delimiter: char) -> Option<Vec<Element>> {
    println!("Processing list of elements: {:?}", input);
    let any_delimiter =
        input.iter().any(|e| if let Element::String(s) = e { s.contains(delimiter) } else { false });
    if !any_delimiter {
        println!("No '{delimiter}' found in brackets, processing elements directly.");
        return None;
    }

    println!("Found '{delimiter}' in list, processing...");
    let mut groups = Vec::new();
    let mut current_group = Vec::new();
    for element in input {
        if let Element::String(str) = element {
            println!("Processing string element: {:?}", str);
            if !str.contains(delimiter) {
                println!("String does not contain '{delimiter}', adding to current group.");
                current_group.push(element.clone());
            } else {
                let parts: Vec<&str> = str.split(delimiter).collect();
                println!("String contains '{delimiter}', splitting into parts: {:?}", parts);
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
            println!("Processing non-string element: {:?}", element);
            current_group.push(element.clone());
        }
    }

    if !current_group.is_empty() {
        groups.push(Element::Brackets(current_group));
    }

    Some(groups)
}
