pub use implementation::get_fun_name_end_of_string;
pub use signature::Signature;

pub mod implementation {
    use crate::{Benchmark, Element, Number, benchmark};
    use regex::Regex;
    use std::mem;
    use std::sync::LazyLock;

    static REGEX_INSERT_PLUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([\w)])-([\w(-])").unwrap());

    fn is_valid_char_for_function_name(c: char) -> bool {
        matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9')
    }

    pub fn get_fun_name_end_of_string(name: &str, allow_first_char_digit: bool) -> String {
        let mut valid_chars_count = 0;
        name.chars()
            .rev()
            .take_while(|&c| is_valid_char_for_function_name(c))
            .for_each(|_| valid_chars_count += 1);
        if valid_chars_count == 0 {
            "".to_owned()
        } else {
            let name = name[name.len() - valid_chars_count..].to_owned();
            if allow_first_char_digit || name.chars().nth(0).is_some_and(|c| !matches!(c, '0'..='9')) {
                name
            } else {
                "".to_owned()
            }
        }
    }

    impl Element {
        pub fn parse(input: &str) -> Result<Self, String> {
            Self::parse_benched(input, &mut Benchmark::new())
        }

        pub fn parse_benched(input: &str, benchmark: &mut Benchmark) -> Result<Self, String> {
            // todo make it possible to write something like `2pi` and have it parsed as `2 * pi`
            let b = benchmark;
            let cow = benchmark!(b, Element::preprocess_string(&input), "preprocess_string_minus");
            let chars = benchmark!(b, cow.chars().collect::<Vec<_>>(), "convert_to_chars");
            let mut start = 0;
            let mut formula =
                benchmark!(b, Element::resolve_brackets(&chars, &mut start), "resolve_brackets");
            benchmark!(b, formula.resolve_functions(), "resolve_functions");
            benchmark!(b, formula.process_plus(), "process_plus");
            benchmark!(b, formula.process_minus(), "process_minus");
            benchmark!(b, formula.process_multiply(), "process_multiply");
            benchmark!(b, formula.process_divide(), "process_divide");
            benchmark!(b, formula.process_minus(), "process_minus_2");
            benchmark!(b, formula.process_pow()?, "process_pow");
            benchmark!(b, formula.process_minus(), "process_minus_3");
            benchmark!(b, formula.process_numbers_and_variables(), "process_numbers_and_variables");
            benchmark!(b, formula.remove_unneeded_outer_brackets(), "remove_unneeded_outer_brackets");
            benchmark!(
                b,
                formula.convert_to_variables_where_possible(),
                "convert_to_variables_where_possible"
            );
            if formula.anything_unparsed() {
                Err("Parts of the formula could not be parsed".to_owned())
            } else {
                Ok(formula)
            }
        }

        /// Step 0
        pub(crate) fn preprocess_string(input: &str) -> String {
            let without_whitespace = input.replace(' ', "").replace('×', "*");
            let mut modified = without_whitespace;
            loop {
                let new = REGEX_INSERT_PLUS.replace_all(&modified, "$1+-$2").to_string();
                if new == modified {
                    break;
                } else {
                    modified = new;
                }
            }
            modified
        }

        /// Step 1
        pub(crate) fn resolve_brackets(input: &[char], start: &mut usize) -> Element {
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
        pub(crate) fn resolve_functions(&mut self) {
            match self {
                Element::Brackets(elements) => {
                    if elements.is_empty() {
                        return;
                    }
                    for i in (0..elements.len() - 1).rev() {
                        let j = i + 1;
                        if let (Element::String(name), Element::Brackets(br_elements)) =
                            (&elements[i], &elements[j])
                        {
                            let name = name.to_string();
                            let function_name = get_fun_name_end_of_string(&name, false);
                            if function_name.is_empty() {
                                continue; // Function names cannot be empty or start with a digit
                            }

                            // create the new function element
                            let arguments = split_list_by_char(br_elements, ',')
                                .unwrap_or_else(|| vec![Element::Brackets(br_elements.clone())]);

                            elements[j] = Element::Function { name: function_name.clone(), arguments };

                            // update or remove the string element
                            let new_str_len = name.len() - function_name.len();
                            if new_str_len == 0 {
                                elements.remove(i);
                            } else {
                                elements[i] = Element::String(name[..new_str_len].to_owned());
                            }
                        }
                    }
                    elements.iter_mut().for_each(Element::resolve_functions);
                },
                Element::Plus(elements)
                | Element::Multiply(elements)
                | Element::Function { arguments: elements, .. } => {
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
        pub(crate) fn process_plus(&mut self) {
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
                | Element::FunctionWithExpression { .. }
                | Element::NumberWithExpression { .. }
                | Element::Pow(..) => {},
            }
        }

        /// Steps 3, 5 and 7
        pub(crate) fn process_minus(&mut self) {
            match self {
                Element::Brackets(elements) => {
                    if let Some(Element::String(str)) = elements.first() {
                        if str.starts_with('-') {
                            let new_string = str[1..].to_owned();
                            if new_string.is_empty() {
                                elements.remove(0);
                            } else {
                                elements[0] = Element::String(new_string);
                            }
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
                        self.process_minus();
                    }
                },
                Element::Negate(element) => {
                    element.process_minus();
                },
                Element::Pow(b, e) => {
                    b.process_minus();
                    e.process_minus();
                },
                Element::Function { arguments, .. } => arguments.iter_mut().for_each(Element::process_minus),
                Element::Variable(_)
                | Element::Number(_)
                | Element::VariableOrFunction(_)
                | Element::FunctionWithExpression { .. }
                | Element::NumberWithExpression { .. } => {},
            }
        }

        /// Step 3
        pub(crate) fn process_multiply(&mut self) {
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
                | Element::Pow(_, _)
                | Element::FunctionWithExpression { .. }
                | Element::NumberWithExpression { .. } => {},
            }
        }

        /// Used in process_divide (Step 4)
        fn invert(&mut self) {
            *self = Element::Pow(
                Box::new(self.clone()),
                Box::new(Element::Negate(Box::new(Element::String("1".to_owned())))),
            );
        }

        /// Step 4
        pub(crate) fn process_divide(&mut self) {
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
                Element::Negate(e) => e.process_divide(),
                Element::Pow(a, b) => {
                    a.process_divide();
                    b.process_divide();
                },
                Element::Variable(_)
                | Element::Number(_)
                | Element::VariableOrFunction(_)
                | Element::FunctionWithExpression { .. }
                | Element::NumberWithExpression { .. } => {},
            }
        }

        /// Step 6
        pub(crate) fn process_pow(&mut self) -> Result<(), String> {
            let create_recursive_pow =
                |element: &mut Element, mut new_elements: Vec<Element>| -> Result<(), String> {
                    let mut working_element =
                        new_elements.pop().ok_or("No element provided for power operation")?;
                    for e in new_elements.into_iter().rev() {
                        working_element = Element::Pow(Box::new(e), Box::new(working_element));
                    }
                    *element = working_element;
                    Ok(())
                };
            match self {
                Element::Brackets(elements) => {
                    if let Some(new_elements) = split_list_by_char(elements, '^') {
                        create_recursive_pow(self, new_elements)?;
                    }
                    match self {
                        Element::Brackets(elements) => {
                            elements.iter_mut().map(Element::process_pow).collect()
                        },
                        Element::Pow(b, e) => {
                            b.process_pow()?;
                            e.process_pow()
                        },
                        _ => Ok(()),
                    }
                },
                Element::Plus(elements) | Element::Multiply(elements) => {
                    elements.iter_mut().map(Element::process_pow).collect()
                },
                Element::Negate(e) => e.process_pow(),
                Element::String(s) => {
                    if let Some(new_elements) = split_string_by_char(s, '^') {
                        create_recursive_pow(self, new_elements)
                    } else {
                        Ok(())
                    }
                },
                Element::Pow(b, p) => {
                    b.process_pow()?;
                    p.process_pow()
                },
                Element::Function { arguments, .. } => {
                    arguments.iter_mut().map(Element::process_pow).collect()
                },
                Element::Variable(_)
                | Element::Number(_)
                | Element::VariableOrFunction(_)
                | Element::FunctionWithExpression { .. }
                | Element::NumberWithExpression { .. } => Ok(()),
            }
        }

        /// Step 8
        pub(crate) fn process_numbers_and_variables(&mut self) {
            match self {
                Element::String(s) => {
                    if let Some(num) = Number::from_string(&s) {
                        *self = Element::Number(num);
                    } else if s.chars().all(is_valid_char_for_function_name) {
                        // If parsing fails, we assume it's a variable or function
                        *self = Element::VariableOrFunction(s.clone());
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
                            if let Some(num) = Number::from_string(&s) {
                                *e = Element::Number(num);
                            } else {
                                *e = Element::VariableOrFunction(s.clone());
                            }
                        } else {
                            e.process_numbers_and_variables();
                        }
                    });
                },
                Element::Variable(_)
                | Element::Number(_)
                | Element::VariableOrFunction(_)
                | Element::FunctionWithExpression { .. }
                | Element::NumberWithExpression { .. } => {},
            }
        }

        /// Step 9
        pub(crate) fn remove_unneeded_outer_brackets(&mut self) {
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
                    elements.iter_mut().for_each(Element::remove_unneeded_outer_brackets);
                    elements.retain(|e| *e != Element::Brackets(vec![]))
                },
                Element::Pow(base, exponent) => {
                    base.remove_unneeded_outer_brackets();
                    exponent.remove_unneeded_outer_brackets();
                },
                Element::Negate(element) => element.remove_unneeded_outer_brackets(),
                Element::Variable(_)
                | Element::Number(_)
                | Element::String(_)
                | Element::VariableOrFunction(_)
                | Element::FunctionWithExpression { .. }
                | Element::NumberWithExpression { .. } => {},
            }
        }

        /// Step 10
        pub(crate) fn convert_to_variables_where_possible(&mut self) {
            match self {
                Element::String(_) | Element::Brackets(_) => {}, // these shouldn't exist at this point
                Element::Plus(elements) | Element::Multiply(elements) => {
                    for arg in elements {
                        if !arg.try_convert_to_variable() {
                            arg.convert_to_variables_where_possible();
                        }
                    }
                },
                Element::Function { arguments, .. } => {
                    arguments.iter_mut().for_each(Element::convert_to_variables_where_possible);
                },
                Element::Pow(a, b) => {
                    if !a.try_convert_to_variable() {
                        a.convert_to_variables_where_possible();
                    }
                    if !b.try_convert_to_variable() {
                        b.convert_to_variables_where_possible();
                    }
                },
                Element::Negate(x) => {
                    if !x.try_convert_to_variable() {
                        x.convert_to_variables_where_possible();
                    }
                },
                Element::FunctionWithExpression { arguments, .. } => {
                    arguments.iter_mut().for_each(|e| {
                        if !e.try_convert_to_variable() {
                            e.convert_to_variables_where_possible();
                        }
                    });
                },
                Element::Variable(_)
                | Element::NumberWithExpression { .. }
                | Element::VariableOrFunction(_)
                | Element::Number(_) => {},
            }
        }

        fn try_convert_to_variable(&mut self) -> bool {
            if let Element::VariableOrFunction(name) = self {
                *self = Element::Variable(name.to_owned());
                true
            } else {
                false
            }
        }

        pub(crate) fn anything_unparsed(&self) -> bool {
            match self {
                Element::Brackets(_) | Element::String(_) => true,
                Element::Plus(elements)
                | Element::Multiply(elements)
                | Element::Function { arguments: elements, .. }
                | Element::FunctionWithExpression { arguments: elements, .. } => {
                    elements.iter().any(Element::anything_unparsed)
                },
                Element::Pow(base, exponent) => base.anything_unparsed() || exponent.anything_unparsed(),
                Element::Negate(element) => element.anything_unparsed(),
                Element::Variable(_)
                | Element::Number(_)
                | Element::VariableOrFunction(_)
                | Element::NumberWithExpression { .. } => false,
            }
        }
    }

    fn list_contains_char(input: &[Element], delimiter: char) -> bool {
        input.iter().any(|e| if let Element::String(s) = e { s.contains(delimiter) } else { false })
    }

    fn split_list_by_char(input: &[Element], delimiter: char) -> Option<Vec<Element>> {
        if !list_contains_char(input, delimiter) {
            return None;
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
        }
        for element in input {
            if let Element::String(str) = element {
                if !str.contains(delimiter) {
                    current_group.push(element.clone());
                } else {
                    let parts: Vec<&str> = str.split(delimiter).collect();
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

pub mod signature {
    use crate::{Element, Symbol};
    use std::cmp::PartialEq;
    use std::collections::{HashMap, HashSet};
    use std::ops::{Deref, DerefMut};

    #[derive(Debug, Clone, PartialEq)]
    pub enum Signature {
        NumberOrFunction,
        Number,
        Function(Vec<Signature>),
        /// This Function only takes numbers as parameters
        FunctionNOrMoreParams(usize),
        Conflicting,
    }

    #[derive(Copy, Clone, Debug, PartialEq)]
    pub enum ParamCount {
        Exactly(usize),
        AtLeast(usize),
    }

    impl ParamCount {
        pub(crate) fn number_would_be_valid(&self, param_count: usize) -> bool {
            match self {
                ParamCount::Exactly(n) => param_count == *n,
                ParamCount::AtLeast(n) => param_count >= *n,
            }
        }
    }

    impl Signature {
        pub(crate) fn accepts_param_count(&self, count: usize) -> bool {
            match self {
                Signature::NumberOrFunction => false,
                Signature::Number => false,
                Signature::Function(params) => params.len() == count,
                Signature::FunctionNOrMoreParams(n) => count >= *n,
                Signature::Conflicting => false,
            }
        }
        fn refine_with(&mut self, new: Self) {
            match (&mut *self, new) {
                (Signature::NumberOrFunction, new) => *self = new,
                (_, Signature::NumberOrFunction) | (Signature::Number, Signature::Number) => {},
                (Signature::Function(args_old), Signature::Function(args_new)) => {
                    if args_old.len() != args_new.len() {
                        *self = Signature::Conflicting;
                        return;
                    }
                    args_old.iter_mut().zip(args_new).for_each(|(a, b)| {
                        a.refine_with(b);
                    });
                    if args_old.iter().any(|a| matches!(a, Signature::Conflicting)) {
                        *self = Signature::Conflicting;
                    }
                },
                (Signature::Function(args_old), Signature::FunctionNOrMoreParams(at_least)) => {
                    if args_old.len() < at_least {
                        *self = Signature::Conflicting;
                        return;
                    }
                    if !args_old.iter().all(|a| matches!(a, Signature::Number | Signature::NumberOrFunction))
                    {
                        *self = Signature::Conflicting;
                        return;
                    }
                    *self = Signature::FunctionNOrMoreParams(at_least)
                },
                (Signature::FunctionNOrMoreParams(at_least), Signature::Function(params)) => {
                    if params.len() < *at_least
                        || !params
                            .iter()
                            .all(|p| matches!(p, Signature::Number | Signature::NumberOrFunction))
                    {
                        *self = Signature::Conflicting;
                    }
                },
                (
                    Signature::FunctionNOrMoreParams(at_least_old),
                    Signature::FunctionNOrMoreParams(at_least_new),
                ) => {
                    if *at_least_old != at_least_new {
                        *self = Signature::Conflicting;
                    }
                },
                (_, _) => {
                    *self = Signature::Conflicting;
                },
            }
        }

        /// True if self is less specific than other and could be refined to match it
        pub(crate) fn could_be(&self, other: &Signature) -> bool {
            let mut refined = self.clone();
            refined.refine_with(other.clone());
            !matches!(refined, Signature::Conflicting)
        }
    }

    #[derive(Clone, Debug)]
    pub struct Signatures(pub(crate) HashMap<String, Signature>);

    impl Deref for Signatures {
        type Target = HashMap<String, Signature>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for Signatures {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl From<HashMap<String, Signature>> for Signatures {
        fn from(value: HashMap<String, Signature>) -> Self {
            Signatures(value)
        }
    }

    impl FromIterator<(String, Signature)> for Signatures {
        fn from_iter<T: IntoIterator<Item = (String, Signature)>>(iter: T) -> Self {
            iter.into_iter().collect::<HashMap<_, _>>().into()
        }
    }

    impl Signatures {
        pub(crate) fn new_empty() -> Self {
            Signatures(HashMap::new())
        }

        pub(crate) fn add_all_undefined_symbols_of_formula(&mut self, element: &Element) {
            match element {
                Element::Plus(elements)
                | Element::Multiply(elements)
                | Element::FunctionWithExpression { arguments: elements, .. } => {
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
                            Element::Number(_)
                            | Element::Plus(_)
                            | Element::Multiply(_)
                            | Element::Pow(..)
                            | Element::Negate(_)
                            | Element::Variable(_) => Signature::Number,
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
                Element::Number(_)
                | Element::NumberWithExpression { .. }
                | Element::Brackets(_)
                | Element::String(_) => {},
            }
        }

        fn insert_or_replace_symbol(&mut self, name: &String, val: Signature) {
            if let Some(entry) = self.get_mut(name) {
                entry.refine_with(val)
            } else {
                self.insert(name.clone(), val);
            }
        }

        pub(crate) fn refine_signature_and_undefined(
            opt_fun_args: &mut OptionalFunctionDeclarationArguments, undefined_signatures: &mut Signatures,
            formula: &Element, already_defined: &Signatures,
        ) -> Result<(), String> {
            let parameter_names =
                opt_fun_args.as_ref().map(|v| v.names.iter().cloned().collect()).unwrap_or(HashSet::new());

            let all_undefined_names =
                undefined_signatures.iter().map(|(n, _)| n).cloned().collect::<HashSet<_>>();
            for name in all_undefined_names {
                if parameter_names.contains(&name) {
                    continue;
                }
                if let Some(already_defined_sig) = already_defined.get(&name) {
                    if !undefined_signatures[&name].could_be(already_defined_sig)
                        && !already_defined_sig.could_be(&undefined_signatures[&name])
                    {
                        return Err(format!(
                            "The signature of {} is not compatible with the already defined signature: undefined: {:?} vs defined: {:?}",
                            name, undefined_signatures[&name], already_defined_sig
                        ));
                    }
                    undefined_signatures.update_signature(&formula, &name, already_defined_sig.clone())
                }
            }
            if let Some(args) = &mut opt_fun_args.0 {
                for (param_name, param_sig) in &mut args.signatures.0 {
                    if let Some(var_sig_in_body) = undefined_signatures.get(param_name) {
                        param_sig.refine_with(var_sig_in_body.clone())
                    }
                }
            }
            undefined_signatures.retain(|n, _| !parameter_names.contains(n));
            undefined_signatures.retain(|n, _| !already_defined.contains_key(n));
            Ok(())
        }

        fn update_signature(&mut self, formula: &Element, element_to_update: &str, new_signature: Signature) {
            let Some(signature) = self.get_mut(element_to_update) else { return };
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
                    self.get_mut(&param_name).unwrap().refine_with(new_arg_signature);
                }
            }
        }
    }

    impl Element {
        pub(crate) fn get_name(&self) -> Option<&str> {
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

        /// Returns a set of Function names and indices, which parameter is equal to the ```name```.
        /// This is used to find all functions that take an Element with the given name as an argument.
        fn list_all_functions_with_argument_variable(&self, name: &str, list: &mut HashSet<(String, usize)>) {
            match self {
                Element::Function { arguments, name: this_name } => {
                    for (i, _) in arguments.iter().enumerate().filter(|(_, e)| e.name_matches(name)) {
                        list.insert((this_name.clone(), i));
                    }
                    arguments.iter().for_each(|a| a.list_all_functions_with_argument_variable(name, list))
                },
                Element::Plus(elements) | Element::Multiply(elements) => {
                    elements.iter().for_each(|a| a.list_all_functions_with_argument_variable(name, list))
                },
                Element::Pow(a, b) => {
                    a.list_all_functions_with_argument_variable(name, list);
                    b.list_all_functions_with_argument_variable(name, list);
                },
                Element::Negate(e) => e.list_all_functions_with_argument_variable(name, list),
                Element::Brackets(_)
                | Element::String(_)
                | Element::Number(_)
                | Element::NumberWithExpression { .. }
                | Element::FunctionWithExpression { .. }
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
							.filter(|e| matches!(e.1, Element::VariableOrFunction(_) | Element::Variable(_)))
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
						.for_each(|a| a.list_all_function_arguments_where_function_has_name(name, list))
				}
				Element::Plus(elements) | Element::Multiply(elements) => elements
					.iter()
					.for_each(|e| e.list_all_function_arguments_where_function_has_name(name, list)),
				Element::Pow(a, b) => {
					a.list_all_function_arguments_where_function_has_name(name, list);
					b.list_all_function_arguments_where_function_has_name(name, list);
				}
				Element::Negate(e) => e.list_all_function_arguments_where_function_has_name(name, list),
				Element::Brackets(_)
				| Element::String(_)
				| Element::Number(_)
				| Element::Variable(_)
                | Element::NumberWithExpression { .. }
				| Element::FunctionWithExpression { .. } // todo I'm not sure if it is correct to ignore this
				| Element::VariableOrFunction(_) => {}
			}
        }
    }

    pub struct SymbolDeclarationData {
        pub(crate) name: String,
        pub(crate) function_args: OptionalFunctionDeclarationArguments,
    }

    pub(crate) struct OptionalFunctionDeclarationArguments(Option<FunctionDeclarationArguments>);

    impl Deref for OptionalFunctionDeclarationArguments {
        type Target = Option<FunctionDeclarationArguments>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl OptionalFunctionDeclarationArguments {
        pub(crate) fn create_symbol(self, formula: Element) -> Symbol {
            Symbol::new(
                self.0
                    .as_ref()
                    .map(|args| Signature::Function(args.get_signatures_in_right_order()))
                    .unwrap_or(Signature::Number),
                self.0.map(|b| b.names),
                formula,
            )
        }
    }

    #[derive(Debug)]
    pub(crate) struct FunctionDeclarationArguments {
        names: Vec<String>,
        signatures: Signatures,
    }

    impl FunctionDeclarationArguments {
        fn get_signatures_in_right_order(&self) -> Vec<Signature> {
            self.names.iter().map(|name| self.signatures.get(name).unwrap().clone()).collect::<Vec<_>>()
        }
    }

    impl SymbolDeclarationData {
        pub(crate) fn from_formula(formula: &Element) -> Result<Self, String> {
            let insert_name;
            let function_args;
            match formula {
                Element::Variable(name) | Element::VariableOrFunction(name) => {
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
                            fn_args.signatures.insert(name.clone(), Signature::NumberOrFunction);
                        } else {
                            return Err("Invalid argument in function signature".to_owned());
                        }
                    }
                    function_args = Some(fn_args)
                },
                _ => {
                    return Err("Invalid formula signature provided".to_owned());
                },
            }
            Ok(Self {
                name: insert_name.to_owned(),
                function_args: OptionalFunctionDeclarationArguments(function_args),
            })
        }

        pub fn get_name(&self) -> &String {
            &self.name
        }
    }

    impl From<&SymbolDeclarationData> for Signature {
        fn from(value: &SymbolDeclarationData) -> Self {
            if let Some(args) = &value.function_args.0 {
                Signature::Function(args.get_signatures_in_right_order())
            } else {
                Signature::Number
            }
        }
    }
}

#[cfg(test)]
pub mod testing {
    use crate::Element;
    use crate::calculation::create_default_context;
    use crate::formula_short::*;
    use astro_float::ctx::Context;

    #[test]
    fn test_parsing_on_manual_formulas() {
        let inputs = [
            (
                "((x/x-x)*-x^x)/(x-x)^-x",
                Some(mul([
                    mul([
                        plus([mul([var("x"), inv(var("x"))]), neg(var("x"))]),
                        neg(pow(var("x"), var("x"))),
                    ]),
                    inv(pow(plus([var("x"), neg(var("x"))]), neg(var("x")))),
                ])),
            ),
            (
                "(x/x+-x)*x^x",
                Some(mul([plus([mul([var("x"), inv(var("x"))]), neg(var("x"))]), pow(var("x"), var("x"))])),
            ),
            ("x/x/x/x", Some(mul([var("x"), inv(var("x")), inv(var("x")), inv(var("x"))]))),
            ("x/x-x", Some(plus([mul([var("x"), inv(var("x"))]), neg(var("x"))]))),
            ("123", Some(num("123"))),
            ("123-1", Some(plus([num("123"), neg(num("1"))]))),
            ("123--1", Some(plus([num("123"), neg(neg(num("1")))]))),
            ("x", Some(var_or_fun("x"))),
            ("1+((2))", Some(plus([num("1"), num("2")]))),
            ("a,b,c", None),
            ("a(a,c)", Some(fun("a", [var_or_fun("a"), var_or_fun("c")]))),
            ("a(a+c)", Some(fun("a", [plus([var("a"), var("c")])]))),
            ("m+a(a,b+c)", Some(plus([var("m"), fun("a", [var_or_fun("a"), plus([var("b"), var("c")])])]))),
            ("fun3(some_fun)", Some(fun("fun3", [var_or_fun("some_fun")]))),
            ("fun(12, fun(1, 2))", Some(fun("fun", [num("12"), fun("fun", [num("1"), num("2")])]))),
            ("fun()", Some(fun("fun", []))),
            ("fun()-fun()", Some(plus([fun("fun", []), neg(fun("fun", []))]))),
            (
                "1+2*3-4/2",
                Some(plus([num("1"), mul([num("2"), num("3")]), neg(mul([num("4"), inv(num("2"))]))])),
            ),
            ("var^--var2", Some(pow(var("var"), neg(neg(var("var2")))))),
            ("11-3-27/60", Some(plus([num(11), neg(num(3)), neg(mul([num(27), inv(num(60))]))]))),
        ];
        println!("Starting formula parsing tests");
        inputs.into_iter().for_each(|(i, o)| debug_formula_parsing_process(i, o));
    }

    fn debug_formula_parsing_process(input: &str, expected_output: Option<Element>) {
        let mut ctx = create_default_context();
        let expected_output = expected_output;
        let cow = Element::preprocess_string(&input);

        println!();
        print_heading("Starting formula parsing");
        println!();

        println!("Input: {}", cow);
        let chars = cow.chars().collect::<Vec<_>>();

        print_heading("0. Bracketize and process operations");

        let mut start = 0;
        let mut brackets = Element::resolve_brackets(&chars, &mut start);

        println!("{}", brackets.get_string(&mut ctx));
        println!("{}", brackets.get_debug_string());
        let mut result = Ok(());
        'processing: {
            debug_print_step("0,5. Resolve functions", &mut brackets, Element::resolve_functions, &mut ctx);
            debug_print_step("1. Processing '+'", &mut brackets, Element::process_plus, &mut ctx);
            debug_print_step("2. Processing '-'", &mut brackets, Element::process_minus, &mut ctx);
            debug_print_step("3. Processing '*'", &mut brackets, Element::process_multiply, &mut ctx);
            debug_print_step("4. Processing '/'", &mut brackets, Element::process_divide, &mut ctx);
            debug_print_step("5. Processing '-' again", &mut brackets, Element::process_minus, &mut ctx);
            debug_print_step(
                "6. Processing '^'",
                &mut brackets,
                |e| {
                    let output = e.process_pow();
                    if output.is_err() {
                        result = output;
                    }
                },
                &mut ctx,
            );
            if result.is_err() {
                break 'processing;
            }
            debug_print_step("7. Processing '-' again", &mut brackets, Element::process_minus, &mut ctx);
            debug_print_step(
                "8. Convert to numbers and variables",
                &mut brackets,
                Element::process_numbers_and_variables,
                &mut ctx,
            );
            debug_print_step(
                "9. Removing unneeded outer brackets",
                &mut brackets,
                Element::remove_unneeded_outer_brackets,
                &mut ctx,
            );
            debug_print_step(
                "10. Convert ambiguous symbols to variables where possible",
                &mut brackets,
                Element::convert_to_variables_where_possible,
                &mut ctx,
            );
        }

        let output = if brackets.anything_unparsed() {
            Err("Parts of the formula could not be parsed".to_owned())
        } else {
            Ok(brackets)
        };
        assert_eq!(output, Element::parse(input));
        assert_eq!(output.ok(), expected_output);
    }

    fn debug_print_step(
        step: &str, element: &mut Element, operation: impl FnOnce(&mut Element), ctx: &mut Context,
    ) {
        print_heading(step);
        operation(element);
        println!("{}", element.get_string(ctx));
        println!("{}", element.get_debug_string());
    }

    fn print_heading(step: &str) {
        println!("##### {}", step);
    }

    mod formula_generation {
        use crate::Element;
        use crate::parsing::testing::debug_formula_parsing_process;
        use crate::printing::Formula;
        use rand::random_range;

        impl Formula {
            fn random_number() -> f64 {
                random_range(0.0..=100.0)
            }

            fn random_var_name() -> String {
                format!("var{}", random_range(1..=10))
            }

            fn generate_random(depth: usize) -> Self {
                match random_range(0..if depth == 0 { 2 } else { 8 }) {
                    0 => Formula::Number(Self::random_number().to_string()),
                    1 => Formula::Variable(Self::random_var_name()),
                    2 => Formula::Plus(
                        (0..random_range(2..=4)).map(|_| Self::generate_random(depth - 1)).collect(),
                    ),
                    3 => Formula::Multiply(
                        (0..random_range(2..=4)).map(|_| Self::generate_random(depth - 1)).collect(),
                    ),
                    4 => Formula::Negate(Box::new(Self::generate_random(depth - 1))),
                    5 => Formula::Pow(
                        Box::new(Self::generate_random(depth - 1)),
                        Box::new(Self::generate_random(depth - 1)),
                    ),
                    6 => Formula::Division(
                        Box::new(Self::generate_random(depth - 1)),
                        Box::new(Self::generate_random(depth - 1)),
                    ),
                    7 => {
                        let name = format!("f{}", random_range(1..=10));
                        let args =
                            (0..random_range(1..=3)).map(|_| Self::generate_random(depth - 1)).collect();
                        Formula::Function { name, arguments: args }
                    },
                    _ => unreachable!(),
                }
            }
        }

        #[test]
        fn test_parsing_randomly_generated_formulas() {
            println!("Starting formula generation tests");
            for _ in 0..100 {
                let formula = Formula::generate_random(3);
                let parsed_result = Element::parse(&formula.to_string());
                if parsed_result.is_err() {
                    println!("Failed to parse: {}", formula);
                    debug_formula_parsing_process(
                        &formula.to_string(),
                        Some(Element::String("Something".to_owned())),
                    )
                }
            }
        }
    }
}
