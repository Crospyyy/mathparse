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
    use anyhow::Result;
    use egui::TextBuffer;

    #[derive(Debug)]
    pub enum LatexConversionError {
        ContainsUnexpectedCharacterInsideFormula(char),
        FailedToTokenize,
        ExpectedArgumentsAfterFunctionName(String),
        MissingClosingBrackets,
    }

    /// Expects latex in format `$formula$` or `formula`
    pub fn convert_from_latex_if_needed(s: &str) -> Option<Result<String, LatexConversionError>> {
        let trimmed = s.trim();
        let is_surrounded = trimmed.starts_with("$") && trimmed.ends_with("$");
        let looks_like_latex = is_surrounded || contains_latex_like_syntax(&trimmed);
        if !looks_like_latex {
            return None;
        };
        Some(convert_to_latex(trimmed))
    }

    pub fn convert_to_latex(s: &str) -> Result<String, LatexConversionError> {
        let mut s = s.trim().to_string();
        if s.starts_with("$") && s.ends_with("$") {
            s = s.trim_start_matches(|c| c == '$').trim_end_matches(|c| c == '$').to_string();
        }
        if s.contains('$') {
            return Err(LatexConversionError::ContainsUnexpectedCharacterInsideFormula('$'));
        }
        preprocess_latex_symbols(&mut s);
        let mut t = Token::tokenize_outer(&mut s.as_str())?;
        t.parse_functions()?;
        Ok(t.convert_latex_to_regular_math(false).into())
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

    #[derive(Debug, Clone)]
    enum Token {
        Group(Vec<Token>),
        Word(String),
        Function(String, Vec<Token>),
    }

    impl Token {
        fn tokenize_outer(s: &mut &str) -> Result<Self, LatexConversionError> {
            let mut elements = vec![];
            if s.is_empty() {
                return Err(LatexConversionError::FailedToTokenize);
            }
            while !s.is_empty() {
                elements.push(Self::tokenize(s)?);
            }
            Ok(Self::Group(elements))
        }

        fn tokenize(s: &mut &str) -> Result<Self, LatexConversionError> {
            *s = s.trim_start();
            let opening_brackets = "{(";
            let closing_brackets = "})";
            let brackets = opening_brackets.to_string() + closing_brackets;
            let operations = "*/+^-".to_string();
            if let Some(c) = s.chars().nth(0) {
                if operations.contains(c) {
                    let string = s[..1].to_string();
                    *s = &s[1..];
                    return Ok(Self::Word(string));
                }
            }
            if s.chars().nth(0).is_some_and(|c| opening_brackets.contains(c)) {
                *s = &s[1..];
                *s = s.trim_start();

                let mut inner = vec![];
                while s.chars().nth(0).is_some_and(|c| !closing_brackets.contains(c)) {
                    inner.push(Self::tokenize(s)?);
                    *s = s.trim_start();
                }
                if !s.chars().nth(0).is_some_and(|c| closing_brackets.contains(c)) {
                    return Err(LatexConversionError::MissingClosingBrackets);
                }
                *s = &s[1..];
                return Ok(Self::Group(inner));
            }
            let string = s
                .chars()
                .take_while(|&c| !brackets.contains(c) && c != ' ' && !operations.contains(c))
                .collect::<String>();
            *s = &s[string.len()..];
            Ok(Self::Word(string))
        }

        fn parse_functions(&mut self) -> Result<(), LatexConversionError> {
            match self {
                Token::Group(inner_tokens) => {
                    inner_tokens.iter_mut().try_for_each(Self::parse_functions)?;
                    let mut new_tokens = vec![];
                    let mut inner_iter = inner_tokens.drain(..);
                    while let Some(token) = inner_iter.next() {
                        new_tokens.push(if let Token::Word(string) = &token {
                            match string.as_str() {
                                r"\sqrt" => {
                                    let arg = inner_iter.next().ok_or(
                                        LatexConversionError::ExpectedArgumentsAfterFunctionName(
                                            string.clone(),
                                        ),
                                    )?;
                                    Token::Function("sqrt".into(), vec![arg])
                                },
                                r"\frac" => {
                                    let num = inner_iter.next().ok_or(
                                        LatexConversionError::ExpectedArgumentsAfterFunctionName(
                                            string.clone(),
                                        ),
                                    )?;
                                    let denom = inner_iter.next().ok_or(
                                        LatexConversionError::ExpectedArgumentsAfterFunctionName(
                                            string.clone(),
                                        ),
                                    )?;
                                    Token::Group(vec![num, Token::Word("/".into()), denom])
                                },
                                _ => token,
                            }
                        } else {
                            token
                        })
                    }
                    drop(inner_iter);
                    *inner_tokens = new_tokens;
                    if inner_tokens.len() == 1 {
                        *self = inner_tokens.pop().unwrap();
                        return Ok(());
                    }
                },
                Token::Word(_) => {},
                _ => {},
            }
            Ok(())
        }

        fn convert_latex_to_regular_math(&self, outer_brackets: bool) -> String {
            match self {
                Token::Group(inner) => {
                    let inner_str = inner
                        .iter()
                        .map(|a| a.convert_latex_to_regular_math(true))
                        .reduce(|a, b| a + " " + &b)
                        .unwrap_or_default();
                    if outer_brackets { format!("({inner_str})") } else { inner_str }
                },
                Token::Word(s) => s.to_string(),
                Token::Function(name, args) => {
                    let args_str = args
                        .iter()
                        .map(|a| a.convert_latex_to_regular_math(false))
                        .reduce(|a, b| a + ", " + &b)
                        .unwrap_or_default();
                    format!("{name}({args_str})")
                },
            }
        }
    }
}
