use crate::ui::Page;
use anyhow::Result;
use egui::Response;
use library::{FormulaStore, RunResult, RunSuccess, Symbol};
use std::collections::HashSet;

pub struct Backend {
    custom_symbols: HashSet<String>,
    formula_store: FormulaStore,
}

impl Backend {
    pub fn new() -> Self {
        let mut store = FormulaStore::new_with_default_symbols();
        define_define_preset_symbols(&mut store).unwrap();
        Self { custom_symbols: HashSet::new(), formula_store: store }
    }

    pub fn get_symbol(&self, name: &str) -> Option<&Symbol> {
        self.formula_store.get_symbol(name)
    }

    pub fn has_custom_symbols(&self) -> bool {
        !self.custom_symbols.is_empty()
    }

    pub fn clear_custom_symbols(&mut self) {
        self.custom_symbols.clear();
        self.formula_store = FormulaStore::new_with_default_symbols(); // todo add logic to remove only custom symbols
    }

    pub(crate) fn formula_store(&self) -> &FormulaStore {
        &self.formula_store
    }

    pub fn dry_run(&mut self, input: &str) -> RunResult {
        self.formula_store.run(input, true)
    }

    pub fn run(&mut self, input: &str) -> RunResult {
        let result = self.formula_store.run(input, false);
        if let RunResult::Ok(RunSuccess::AddedSymbol(s)) = &result {
            self.custom_symbols.insert(s.name().clone());
        }
        result
    }
}

fn define_define_preset_symbols(store: &mut FormulaStore) -> Result<()> {
    let preset_symbols = [
        "speed_of_sound_mps = 343",
        "speed_of_light_mps = 299_792_458",
        "kw_to_ps = 1.35962",
        "km_to_miles = 0.6214",
        "liter_to_gallons = 0.264172",
        "joule_to_wh = 1/3600",
        "water_heat_capacity_j_per_g = 4.184",
    ];
    for symbol in preset_symbols {
        store.add_symbol_from_string(symbol, false)?;
    }
    Ok(())
}

pub enum UiInteraction {
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
    use anyhow::Result;
    use egui::TextBuffer;
    use library::only_in_debug;
    use library::{debug_print, get_fun_name_end_of_string};

    #[derive(Debug, PartialEq)]
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
        Some(convert_latex_to_math(trimmed))
    }

    pub fn convert_latex_to_math(s: &str) -> Result<String, LatexConversionError> {
        let mut s = s.trim().to_string();
        if s.starts_with("$") && s.ends_with("$") {
            s = s.trim_start_matches(|c| c == '$').trim_end_matches(|c| c == '$').to_string();
        }
        if s.contains('$') {
            return Err(LatexConversionError::ContainsUnexpectedCharacterInsideFormula('$'));
        }
        preprocess_latex_symbols(&mut s);
        debug_print!("processed string: {}", s);
        let mut t = LatexToken::tokenize_outer(&mut s.as_str())?;
        debug_print!("tokenized: {:?}", t);
        t.parse_functions()?;
        debug_print!("parsed functions: {:?}", t);
        Ok(t.to_string(false).into())
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
    enum LatexToken {
        Group(Vec<LatexToken>),
        Word(String),
        Function(String, Vec<LatexToken>),
    }

    impl LatexToken {
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
            let operations = "*/+^-=".to_string();
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
                LatexToken::Group(inner_tokens) => {
                    let mut new_tokens = vec![];
                    let mut inner_iter = inner_tokens.drain(..).peekable();
                    while let Some(mut token) = inner_iter.next() {
                        if let LatexToken::Word(string) = &token {
                            match string.as_str() {
                                r"\sqrt" => {
                                    let arg = inner_iter.next().ok_or(
                                        LatexConversionError::ExpectedArgumentsAfterFunctionName(
                                            string.clone(),
                                        ),
                                    )?;
                                    token = LatexToken::Function(
                                        "sqrt".into(),
                                        vec![LatexToken::Group(vec![arg])],
                                    );
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
                                    token = LatexToken::Group(vec![num, LatexToken::Word("/".into()), denom]);
                                },
                                _ => {
                                    if !get_fun_name_end_of_string(string, false).is_empty() {
                                        if matches!(
                                            inner_iter.peek(),
                                            Some(LatexToken::Function(..) | LatexToken::Group(_))
                                        ) {
                                            let arg = inner_iter.next().unwrap();
                                            token = LatexToken::Function(string.clone(), vec![arg]);
                                        }
                                    }
                                },
                            }
                        }
                        new_tokens.push(token)
                    }
                    drop(inner_iter);
                    *inner_tokens = new_tokens;

                    inner_tokens.iter_mut().try_for_each(Self::parse_functions)?;

                    if inner_tokens.len() == 1 {
                        *self = inner_tokens.pop().unwrap();
                        return Ok(());
                    }
                },
                LatexToken::Word(_) => {},
                LatexToken::Function(_, args) => {
                    args.iter_mut().try_for_each(Self::parse_functions)?;
                },
            }
            Ok(())
        }

        fn to_string(&self, outer_brackets: bool) -> String {
            match self {
                LatexToken::Group(inner) => {
                    let inner_str =
                        inner.iter().map(|a| a.to_string(true)).reduce(|a, b| a + &b).unwrap_or_default();
                    if outer_brackets { format!("({inner_str})") } else { inner_str }
                },
                LatexToken::Word(s) => s.to_string(),
                LatexToken::Function(name, args) => {
                    let args_str = args
                        .iter()
                        .map(|a| a.to_string(false))
                        .reduce(|a, b| a + ", " + &b)
                        .unwrap_or_default();
                    format!("{name}({args_str})")
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::latex_conversion::convert_latex_to_math;

    #[test]
    fn test_convert_latex_to_math() {
        let data = [
            (r"\frac{1}{2}", "1/2"),
            (r"\sqrt{4}", "sqrt(4)"),
            (r"\sqrt{\frac{1}{4}}", "sqrt(1/4)"),
            (r"$f(x) = x^2$", "f(x)=x^2"),
            (r"$f(x,y) = x^2+y$", "f(x,y)=x^2+y"),
            (r"3 \cdot 4 + 5 \div 2", "3*4+5/2"),
            (r"\pi * r^2", "pi*r^2"),
            (r"\frac{(a+b)^2}{c-d} = \sqrt{e^2 + f^2}", "(((a+b)^2)/(c-d))=sqrt(e^2+f^2)"),
            (
                r"\left(\frac{x^2 + 1}{y - 3}\right)^3 + \sqrt{\frac{z^4}{x+y}}",
                "((x^2+1)/(y-3))^3+sqrt((z^4)/(x+y))",
            ),
            (r"\frac{\sqrt{(m+n)^2 + (p-q)^2}}{r+s}", "sqrt((m+n)^2+(p-q)^2)/(r+s)"),
            (r"\frac{\left(\frac{a}{b} + \frac{c}{d}\right)^2}{\sqrt{e+f}}", "(((a/b)+(c/d))^2)/sqrt(e+f)"),
        ];
        for (input, expected) in data {
            let result = convert_latex_to_math(input).unwrap();
            assert_eq!(result, expected);
        }
    }
}
