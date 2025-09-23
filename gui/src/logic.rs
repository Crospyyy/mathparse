use crate::ui::Page;
use egui::Response;
use library::Symbol;

pub enum UiStateInfo {
    TopInputChanged,
    TopInputSubmit,
    RoundingAccuracyChanged,
    RequestAutocompletion { cursor_pos: usize, input_term: String, complete_to: String, response: Response },
    SelectPage(Page),
    ClearCustomSymbols,
    ClearHistory,
}

pub fn determine_longest_common_start(names: &[(&String, &Symbol)]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let common_start = names[0].0.to_string();
    let mut longest_common = common_start.len();
    for name in names.iter().map(|(n, _)| n).skip(1) {
        if longest_common == 0 {
            return String::new();
        }
        let max = longest_common.min(name.len());
        longest_common = max;
        for i in 0..max {
            if common_start.chars().nth(i) != name.chars().nth(i) {
                longest_common = i;
                break;
            }
        }
    }
    common_start[..longest_common].to_string()
}

pub mod latex_conversion {
    use crate::debug_print;
    use anyhow::{Result, anyhow};
    use egui::TextBuffer;

    /// Expects latex in format `$formula$` or `formula`
    pub fn convert_from_latex_if_needed(s: &str) -> Result<Option<String>> {
        let mut s = s.trim().to_string();
        // input: $formula$ or formula
        let is_surrounded = s.starts_with("$") && s.ends_with("$");
        if !is_surrounded && !contains_latex_like_syntax(&mut s) {
            return Ok(None);
        };
        if is_surrounded {
            s = s.trim_start_matches(|c| c == '$').trim_end_matches(|c| c == '$').to_string();
        }
        if s.contains('$') {
            return Err(anyhow!("Formula contains '$' inside"));
        }
        preprocess_latex_symbols(&mut s);
        let t = Token::tokenize_outer(&mut s).ok_or(anyhow!("Failed to tokenize"))?;
        dbg!(&t);
        let new_string = t.convert_latex_to_regular_math(false).ok_or(anyhow!("Regex is invalid"))?;
        debug_print!("Converted to regular: {new_string}");

        Ok(Some(new_string))
    }

    fn contains_latex_like_syntax(s: &str) -> bool {
        let matchers = [r"\cdot", r"\div", r"\frac{", r"\sqrt{", r"\pi"];
        matchers.iter().any(|m| s.contains(m))
    }

    fn preprocess_latex_symbols(s: &mut String) {
        let conversions = [(r"\cdot", "*"), (r"\div", "/"), (r"\pi", "pi"), (r"\left", ""), (r"\right", "")];
        for (from, to) in conversions {
            *s = s.replace(from, to);
        }
    }

    #[derive(Debug)]
    enum Token {
        Group(Vec<Token>),
        Word(String),
    }

    impl Token {
        fn tokenize_outer(s: &mut String) -> Option<Self> {
            let mut elements = vec![];
            if s.is_empty() {
                return None;
            }
            while !s.is_empty() {
                elements.push(Self::tokenize(s)?);
            }
            Self::Group(elements).into()
        }
        fn tokenize(s: &mut String) -> Option<Self> {
            dbg!(&s);
            *s = s.trim_start().to_string();
            let opening_brackets = ['{', '('];
            let closing_brackets = ['}', ')'];
            let brackets: Vec<_> = opening_brackets.into_iter().chain(closing_brackets.into_iter()).collect();
            let operations = ['*', '/', '+', '^', '-'];
            if let Some(c) = s.chars().nth(0) {
                if operations.contains(&c) {
                    let string = s[..1].to_string();
                    s.remove(0);
                    return Self::Word(string).into();
                }
            }
            if s.chars().nth(0).is_some_and(|c| opening_brackets.contains(&c)) {
                s.remove(0);
                *s = s.trim_start().to_string();

                let mut inner = vec![];
                while !s.chars().nth(0).is_some_and(|c| closing_brackets.contains(&c)) {
                    if s.is_empty() {
                        return None;
                    }
                    inner.push(Self::tokenize(s)?);
                    *s = s.trim_start().to_string();
                }
                if !s.chars().nth(0).is_some_and(|c| closing_brackets.contains(&c)) {
                    return None;
                }
                s.remove(0);
                return Self::Group(inner).into();
            }
            let string = s
                .chars()
                .take_while(|c| !brackets.contains(c) && *c != ' ' && !operations.contains(c))
                .collect::<String>();
            *s = s[string.len()..].to_owned();
            Self::Word(string).into()
        }

        fn convert_latex_to_regular_math(self, outer_brackets: bool) -> Option<String> {
            match self {
                Token::Group(mut inner) => {
                    if inner.len() == 1 {
                        return inner.pop().unwrap().convert_latex_to_regular_math(outer_brackets);
                    }
                    let string = Self::create_group_string(inner, outer_brackets)?;
                    string.into()
                },
                Token::Word(s) => s.into(),
            }
        }

        fn create_group_string(inner: Vec<Token>, outer_brackets: bool) -> Option<String> {
            let mut inner_str_arr = vec![];
            let mut inner_iter = inner.into_iter().peekable();
            while let Some(next) = inner_iter.next() {
                match next {
                    Token::Word(s) => match s.as_str() {
                        r"\frac" => {
                            let num = inner_iter.next()?.convert_latex_to_regular_math(true)?;
                            let denom = inner_iter.next()?.convert_latex_to_regular_math(true)?;
                            if !outer_brackets || (inner_str_arr.is_empty() && inner_iter.peek().is_none()) {
                                inner_str_arr.push(format!("{} / {}", num, denom));
                            } else {
                                inner_str_arr.push(format!("({} / {})", num, denom));
                            }
                        },
                        r"\sqrt" => {
                            let inner = inner_iter.next()?.convert_latex_to_regular_math(false)?;
                            inner_str_arr.push(format!("sqrt({inner})"));
                        },
                        _ => inner_str_arr.push(s),
                    },
                    Token::Group(_) => {
                        inner_str_arr.push(next.convert_latex_to_regular_math(true)?);
                    },
                }
            }
            let string = inner_str_arr.join(" ");
            Some(if outer_brackets && inner_str_arr.len() > 1 { format!("({string})") } else { string })
        }
    }
}
