use crate::new::{MatchInput, MatchOutput};
use crate::old::outer;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{TokenStreamExt, quote};

/// # Usage
/// ## Element matcher \[EM\]
/// - **Expression**
///     - `{some_expr}` Only match if the compared element is equal to the result of `some_expr`
///     - `{some_expr}..` Only match if all the compared elements are equal to the elements inside the array `some_expr`
/// - **Get Element**
///     - `x` Get the value of one Element
///     - `x..` Get the values of all the Elements as an array
///     - `x, x, x` Get the values of multiple Elements as an array
/// - **Match Any**
///     - `_` Match any element
/// - **Number**
///     - `num` Match any number element
///     - `num([NM])` Match the inner value of the number element
/// - **Variable**
///     - `var` Match any variable element
///     - `var([SM])` Match the inner value of the variable element
/// - **Negate**
///     - `neg` Match any negate element
///     - `neg([EM])` Match the inner element of the negate element
/// - **Plus**
///     - `plus` Match any plus element
///     - `plus([EM]..)` Match the elements inside the plus element
/// - **Multiply**
///     - `mul` Match any multiply element
///     - `mul([EM]..)` Match the elements inside the multiply element
/// - **Pow**
///     - `pow` Match any power element
///     - `pow([EM]..)` Match the base and exponent of the power element
/// - **Function**
///     - `fun` Match any function element
///     - `fun([SM], [EM]..)` Match the function name and the elements inside the function element
/// ## Number matcher \[NM\]
/// - **Get Number**
///     - `x` Get the value of one number element
/// - **Compare Number**
///     - `{some_expr}` Only match if the compared number is equal to the result of `some_expr`
/// ## String matcher \[SM\]
/// - **Get String**
///     - `x` get the value of one string element
/// - **Compare String**
///     - `{some_expr}` only match if the compared string is equal to the result of `some_expr`
#[proc_macro]
pub fn match_formula(item: TokenStream) -> TokenStream {
    outer(item)
}

#[proc_macro]
pub fn return_tokens(item: TokenStream) -> TokenStream {
    let strings: Vec<_> = item.into_iter().map(|e| format!("{:?}", e)).collect();
    let string = strings.join(", ");
    quote! {#string}.into()
}

mod old {
    use crate::new::{
        MatchElement, MatchInput, MatchOutput, create_outputs, create_var_name, split_by_comma_2,
    };
    use crate::old;
    use proc_macro::TokenStream;
    use proc_macro2::{Ident, Span, TokenStream as TokenStream2, TokenTree};
    use quote::{TokenStreamExt, quote};
    use std::str::FromStr;

    pub(super) fn outer(item: TokenStream) -> TokenStream {
        let input: TokenStream2 = item.into();

        let MatchInput { formula, matcher } = MatchInput::parse(input);
        let MatchOutput { tokens, var_count } = old::generate_match(formula, matcher);
        let mut tokens = quote! { (||#tokens)() };
        if var_count == 0 {
            tokens.append_all(quote! { .is_some() })
        }
        tokens.into()
    }

    impl MatchInput {
        pub(super) fn parse(input: TokenStream2) -> MatchInput {
            let input_args = split_by_comma_2(input);
            if input_args.len() != 2 {
                panic!("expected exactly two arguments");
            }
            let mut input_args_iter = input_args.into_iter();

            let formula = input_args_iter.next().unwrap();
            let match_expr = input_args_iter.next().unwrap();

            let formula_ts: TokenStream2 = formula.into_iter().collect();

            MatchInput { formula: formula_ts, matcher: match_expr }
        }
    }

    fn parse_element_matcher(match_expr: TokenStream2) -> (Ident, Option<Vec<Vec<TokenTree>>>) {
        let match_expr = match_expr.into_iter().collect::<Vec<_>>();
        // ident
        if match_expr.len() > 2 {
            panic!("Expected no more than two tokens in match expression");
        }
        let match_ident = match match_expr.get(0) {
            Some(TokenTree::Ident(i)) => i.clone(),
            _ => panic!("expected identifier"),
        };
        let inner_elements = match_expr.get(1).map(|tt| match tt {
            TokenTree::Group(g) => split_by_comma(g.stream()),
            _ => panic!("unexpected token after identifier"),
        });
        (match_ident, inner_elements)
    }

    fn split_by_comma(ts: TokenStream2) -> Vec<Vec<TokenTree>> {
        // initialisiere ergebnisvektor
        let mut result = Vec::new();
        // sammle tokens bis zum kommatrennzeichen
        let mut segment = Vec::new();
        for tt in ts.into_iter() {
            if let TokenTree::Punct(p) = &tt {
                if p.as_char() == ',' {
                    result.push(segment);
                    segment = Vec::new();
                    continue;
                }
            }
            segment.push(tt);
        }
        // letztes segment hinzufügen wenn nicht leer
        if !segment.is_empty() {
            result.push(segment);
        }
        result
    }

    pub(super) fn generate_match(formula: TokenStream2, matcher: TokenStream2) -> MatchOutput {
        let (match_ident, inner_elements) = parse_element_matcher(matcher);
        if match_ident.to_string() == "_" && inner_elements.is_none() {
            return MatchOutput { tokens: quote! { Some(()) }, var_count: 0 };
        }

        let mut var_count = 0usize;

        let name = match_ident.to_string();
        let match_element = MatchElement::from_str(&name).expect("unexpected element to match on");
        let match_stream: TokenStream2 = if let Some(operation_args) = inner_elements {
            match match_element {
                MatchElement::Number => {
                    if operation_args.len() != 1 || operation_args[0].len() != 1 {
                        panic!("expected exactly one inner element for num");
                    }
                    match &operation_args[0][0] {
                        TokenTree::Ident(i) => {
                            if i.to_string() != "x" {
                                panic!("expected identifier 'x' as inner element for num, got '{}'", i);
                            }
                            var_count += 1;
                            quote! { Element::Number(n) => Some(n) }
                        }
                        TokenTree::Literal(l) => {
                            quote! { Element::Number(n) => (n == #l).then_some(()) }
                        }
                        _ => panic!("expected identifier or literal as inner element for num"),
                    }
                }
                MatchElement::Plus => {
                    let expected_elements = operation_args.len();
                    let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                    let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                        VariablesOutput::create(args.clone());
                    let outputs_ts_stream = create_outputs(&var_counts);
                    var_count += var_counts.iter().copied().sum::<usize>();
                    quote! {
                    Element::Plus(inputs) => {
                        if inputs.len() != #expected_elements {
                            return None;
                        }
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
                }
                MatchElement::Multiply => {
                    let expected_elements = operation_args.len();
                    let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                    let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                        VariablesOutput::create(args.clone());
                    let outputs_ts_stream = create_outputs(&var_counts);
                    var_count += var_counts.iter().copied().sum::<usize>();
                    quote! {
                    Element::Multiply(inputs) => {
                        if inputs.len() != #expected_elements {
                            return None;
                        }
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
                }
                MatchElement::Pow => {
                    if operation_args.len() != 2 {
                        panic!("expected exactly two inner elements for pow");
                    }
                    let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                    let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                        VariablesOutput::create(args.clone());
                    let outputs_ts_stream = create_outputs(&var_counts);
                    var_count += var_counts.iter().copied().sum::<usize>();
                    quote! {
                    Element::Pow(b, e) => {
                        let inputs = [b.as_ref(), e.as_ref()];
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
                }
                MatchElement::Variable => {
                    if operation_args.len() != 1 {
                        panic!("expected exactly one inner element for var");
                    }
                    let inner = operation_args[0].iter().map(|e| e.to_string()).collect::<String>();
                    if inner == "x" {
                        var_count += 1;
                        quote! { Element::Variable(n) => Some(n) }
                    } else {
                        if is_in_quotes(&inner) {
                            let inner_without_qoutes = inner[1..inner.len() - 1].to_string();
                            quote! { Element::Variable(n) => { (n == #inner_without_qoutes).then_some(()) } }
                        } else {
                            panic!(
                                "expected identifier 'x' or a string literal as inner element for var, got '{}'",
                                inner
                            );
                        }
                    }
                }
                MatchElement::Function => {
                    let expected_elements = operation_args.len();
                    let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                    let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                        VariablesOutput::create(args.clone());
                    let outputs_ts_stream = create_outputs(&var_counts);
                    var_count += var_counts.iter().copied().sum::<usize>();

                    quote! {
                    Element::Function { arguments: inputs, .. } => {
                        if inputs.len() != #expected_elements {
                            return None;
                        }
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
                }
                MatchElement::Negate => {
                    if operation_args.len() != 1 {
                        panic!("expected exactly one inner element for neg");
                    }
                    let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                    let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                        VariablesOutput::create(args.clone());
                    let outputs_ts_stream = create_outputs(&var_counts);
                    var_count += var_counts.iter().copied().sum::<usize>();

                    quote! {
                    Element::Negate(element) => {
                        let inputs = [element.as_ref()];
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
                }
            }
        } else {
            let match_name = match_element.as_pattern();
            quote! {
            Element::#match_name {..} => Some(())
        }
        }
            .into();
        let tokens = quote! {{
            match &#formula {
                #match_stream,
                _ => None,
            }
        }};
        MatchOutput { tokens, var_count }
    }

    struct VariablesOutput {
        tokens: TokenStream2,
        var_counts: Vec<usize>,
    }

    impl VariablesOutput {
        fn create(ts: impl IntoIterator<Item=TokenStream2>) -> VariablesOutput {
            let mut var_counts = Vec::new();
            let tokens = ts
                .into_iter()
                .enumerate()
                .map(|(i, ts)| {
                    let var_name = TokenStream2::from(TokenTree::Ident(Ident::new(
                        &create_var_name(i),
                        Span::call_site(),
                    )));
                    if ts.to_string() == "x" {
                        var_counts.push(1);
                        quote! {
                            let #var_name = inputs[#i];
                        }
                    } else {
                        let inner_call = generate_match(quote! {inputs[#i]}, ts.clone());
                        var_counts.push(inner_call.var_count);
                        let tokens_inner_call = inner_call.tokens;
                        if inner_call.var_count != 0 {
                            quote! {
                                let #var_name = #tokens_inner_call?;
                            }
                        } else {
                            quote! {
                                #tokens_inner_call?;
                            }
                        }
                    }
                })
                .collect();
            VariablesOutput { tokens, var_counts }
        }
    }

    fn is_in_quotes(str: &String) -> bool {
        let chars = str.chars().collect::<Vec<_>>();
        (str.starts_with('"') && str.ends_with('"') | (str.starts_with('\'') && str.ends_with('\'')))
            && str.len() > 2
            && chars[1..chars.len() - 1].iter().all(|c| !"\"'".contains(*c))
    }
}

mod new {
    use proc_macro::TokenStream as TokenStreamOld;
    use proc_macro2::{Ident, Span, TokenStream, TokenTree};
    use quote::{TokenStreamExt, quote};
    use std::str::FromStr;

    pub(super) struct MatchInput {
        pub(crate) formula: TokenStream,
        pub(crate) matcher: TokenStream,
    }

    fn create_flattened_var(name: &str, count: usize) -> String {
        (0..count).map(|i| format!("{}.{}", name, i)).collect::<Vec<_>>().join(", ")
    }

    pub(super) fn create_outputs(var_counts: &[usize]) -> TokenStream {
        let string = var_counts
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(i, var_count)| {
                (var_count != 0).then(|| {
                    let name = create_var_name(i);
                    if var_count == 1 { name } else { create_flattened_var(&name, var_count) }
                })
            })
            .collect::<Vec<_>>()
            .join(", ");
        TokenStream::from_str(&format!("Some(({}))", string)).expect("could not create output")
    }

    pub(super) fn create_var_name(i: usize) -> String {
        format!("var_{}", i)
    }

    pub(super) struct MatchOutput {
        pub(crate) tokens: TokenStream,
        pub(crate) var_count: usize,
    }

    impl MatchOutput {
        fn no_output(tokens: TokenStream) -> Self {
            Self { tokens, var_count: 0 }
        }

        fn with_output(tokens: TokenStream, var_count: usize) -> Self {
            Self { tokens, var_count }
        }

        fn generate_variable(&self, index: usize) -> TokenStream {
            if self.var_count != 0 {
                let var_name = TokenStream::from_str(&create_var_name(index)).unwrap();
                quote! { let #var_name = #{self.tokens}?; }
            } else {
                quote! { #{self.tokens}?; }
            }
        }

        fn create_code(matches: &[ElementMatcher], formula: TokenStream) -> MatchOutput {
            let match_tokens: Vec<_> =
                matches.iter().enumerate().map(|(i, m)| m.perform_match(quote! { #formula[#i] })).collect();
            let variables =
                match_tokens.iter().enumerate().map(|(i, m)| m.generate_variable(i)).collect::<TokenStream>();
            let var_count = match_tokens.iter().map(|m| m.var_count).sum();
            let outputs = create_outputs(&match_tokens.iter().map(|m| m.var_count).collect::<Vec<_>>());
            MatchOutput::with_output(
                quote! {
                    #variables
                    #outputs
                },
                var_count,
            )
        }
        fn create_code_variable_input_len(matcher: ElementMatcher) -> MatchOutput {
            let match_output = matcher.perform_match(quote! { i });
            if match_output.var_count == 0 {
                MatchOutput::no_output(quote! {
                    for i in inputs {
                        #{match_output.tokens}?;
                    }
                    Some(())
                })
            } else {
                MatchOutput::with_output(
                    quote! {
                        Some(inputs.iter().map(|i| {
                            #{match_output.tokens}?
                        }).collect::<Option<Vec<_>>>()?);
                    },
                    1,
                )
            }
        }
    }

    pub enum MatchElement {
        Number,
        Negate,
        Plus,
        Multiply,
        Pow,
        Variable,
        Function,
    }

    impl MatchElement {
        pub(crate) fn from_str(s: &str) -> Option<Self> {
            match s {
                "num" => Some(MatchElement::Number),
                "plus" => Some(MatchElement::Plus),
                "mul" => Some(MatchElement::Multiply),
                "pow" => Some(MatchElement::Pow),
                "var" => Some(MatchElement::Variable),
                "fun" => Some(MatchElement::Function),
                "neg" => Some(MatchElement::Negate),
                _ => None,
            }
        }

        fn as_str(&self) -> &str {
            match self {
                MatchElement::Number => "num",
                MatchElement::Negate => "neg",
                MatchElement::Plus => "plus",
                MatchElement::Multiply => "mul",
                MatchElement::Pow => "pow",
                MatchElement::Variable => "var",
                MatchElement::Function => "fun",
            }
        }

        pub(crate) fn as_pattern(&self) -> TokenTree {
            let name = match self {
                MatchElement::Number => "Number",
                MatchElement::Negate => "Negate",
                MatchElement::Plus => "Plus",
                MatchElement::Multiply => "Multiply",
                MatchElement::Pow => "Pow",
                MatchElement::Variable => "Variable",
                MatchElement::Function => "Function",
            };
            TokenTree::Ident(Ident::new(name, Span::call_site()))
        }
    }

    enum SingleOrMultipleElementMatcher {
        /// like `x` or `x, x`
        EachMatch(Vec<ElementMatcher>),
        /// like `x..`
        Flatten(ElementMatcher),
    }

    enum ElementMatcher {
        /// _
        Any,
        /// x
        X,
        /// some_expr
        Expression(TokenStream),
        /// num / pow / plus
        WithoutInner(MatchElement),
        Number(NumberMatcher),
        Variable(StringMatcher),
        Negate(Box<ElementMatcher>),
        Plus(Box<SingleOrMultipleElementMatcher>),
        Multiply(Box<SingleOrMultipleElementMatcher>),
        Pow(Box<SingleOrMultipleElementMatcher>),
        Function(StringMatcher, Box<SingleOrMultipleElementMatcher>),
    }

    trait Matcher {
        fn perform_match(&self, formula: TokenStream) -> MatchOutput;
    }

    impl Matcher for ElementMatcher {
        fn perform_match(&self, formula: TokenStream) -> MatchOutput {
            match self {
                ElementMatcher::Any => MatchOutput::no_output(quote! { Some(()) }),
                ElementMatcher::X => MatchOutput::with_output(quote! { Some(#formula) }, 1),
                ElementMatcher::Expression(expr) => {
                    MatchOutput::no_output(quote! { (#formula == #expr).then_some(()) })
                },
                ElementMatcher::WithoutInner(element) => {
                    let element_string = element.as_pattern();
                    MatchOutput::no_output(quote! { matches!(#formula, #element_string {..}).then_some(()) })
                },
                ElementMatcher::Number(n) => {
                    let inner = n.perform_match(quote! { n.as_ref() });
                    let inner_tokens = inner.tokens;
                    MatchOutput::with_output(
                        quote! { if let Element::Number(n) = #formula { #inner_tokens? } else { None } },
                        inner.var_count,
                    )
                },
                ElementMatcher::Variable(n) => {
                    let inner = n.perform_match(quote! { n });
                    let inner_tokens = inner.tokens;
                    MatchOutput::with_output(
                        quote! { if let Element::Variable(n) = #formula { #inner_tokens? } else { None } },
                        inner.var_count,
                    )
                },
                ElementMatcher::Negate(n) => {
                    let inner = n.perform_match(quote! { n.as_ref() });
                    let inner_tokens = inner.tokens;
                    MatchOutput::with_output(
                        quote! { if let Element::Negate(n) = #formula { #inner_tokens? } else { None } },
                        inner.var_count,
                    )
                },
                ElementMatcher::Plus(n) => {
                    let inner = n.as_ref().perform_match(quote! { inputs });
                    let inner_tokens = inner.tokens;
                    MatchOutput::with_output(
                        quote! { if let Element::Plus(inputs) = #formula { #inner_tokens? } else { None } },
                        inner.var_count,
                    )
                },
                ElementMatcher::Multiply(n) => {
                    let inner = n.as_ref().perform_match(quote! { inputs });
                    let inner_tokens = inner.tokens;
                    MatchOutput::with_output(
                        quote! { if let Element::Multiply(inputs) = #formula { #inner_tokens? } else { None } },
                        inner.var_count,
                    )
                },
                ElementMatcher::Pow(n) => {
                    let inner = n.as_ref().perform_match(quote! { inputs });
                    let inner_tokens = inner.tokens;
                    MatchOutput::with_output(
                        quote! {
                        if let Element::Pow(b, e) = #formula {
                            let inputs = [b.as_ref(), e.as_ref()];
                            #inner_tokens
                        } else { None } },
                        inner.var_count,
                    )
                },
                ElementMatcher::Function(s, n) => {
                    let name_inner = s.perform_match(quote! { name });
                    let args_inner = n.perform_match(quote! { arguments });
                    let name_var = name_inner.generate_variable(0);
                    let args_var = args_inner.generate_variable(1);
                    let outputs = create_outputs(&[name_inner.var_count, args_inner.var_count]);

                    MatchOutput::with_output(
                        quote! {
                        if let Element::Function { name, arguments } = #formula {
                            #name_var
                            #args_var
                            #outputs
                        } else { None } },
                        name_inner.var_count + args_inner.var_count,
                    )
                },
            }
        }
    }

    impl Matcher for SingleOrMultipleElementMatcher {
        fn perform_match(&self, formula: TokenStream) -> MatchOutput {
            match self {
                SingleOrMultipleElementMatcher::EachMatch(matches) => {
                    MatchOutput::create_code(matches, formula)
                },
                SingleOrMultipleElementMatcher::Flatten(matcher) => {
                    let inner = matcher.perform_match(quote! { e });
                    let tokens = inner.tokens;
                    MatchOutput::with_output(
                        quote! {
                            #formula.iter().map(|e| #tokens).collect::<Option<Vec<_>>>()
                        },
                        1,
                    )
                },
            }
        }
    }

    enum NumberMatcher {
        GetValue,
        CompareTo(TokenStream),
    }

    impl Matcher for NumberMatcher {
        fn perform_match(&self, formula: TokenStream) -> MatchOutput {
            match self {
                NumberMatcher::GetValue => MatchOutput::with_output(formula, 1),
                NumberMatcher::CompareTo(comp_val) => MatchOutput::no_output(quote! {
                    (#formula == #comp_val).then_some(())
                }),
            }
        }
    }

    enum StringMatcher {
        GetValue,
        CompareTo(TokenStream),
    }

    impl Matcher for StringMatcher {
        fn perform_match(&self, formula: TokenStream) -> MatchOutput {
            match self {
                StringMatcher::GetValue => MatchOutput::with_output(formula, 1),
                StringMatcher::CompareTo(comp_val) => MatchOutput::no_output(quote! {
                    (#formula == #comp_val).then_some(())
                }),
            }
        }
    }

    pub(super) fn split_by_comma_2(ts: TokenStream) -> Vec<TokenStream> {
        // initialisiere ergebnisvektor
        let mut result = Vec::new();
        // sammle tokens bis zum kommatrennzeichen
        let mut segment = Vec::new();
        for tt in ts.into_iter() {
            if let TokenTree::Punct(p) = &tt {
                if p.as_char() == ',' {
                    result.push(segment.into_iter().collect());
                    segment = Vec::new();
                    continue;
                }
            }
            segment.push(tt);
        }
        // letztes segment hinzufügen wenn nicht leer
        if !segment.is_empty() {
            result.push(segment.into_iter().collect());
        }
        result
    }

    fn parse_element_matcher(match_expr: TokenStream) -> (Ident, Option<TokenStream>) {
        let match_expr = match_expr.into_iter().collect::<Vec<_>>();
        // ident
        if match_expr.len() > 2 {
            panic!("Expected no more than two tokens in match expression");
        }
        let match_ident = match match_expr.get(0) {
            Some(TokenTree::Ident(i)) => i.clone(),
            _ => panic!("expected identifier"),
        };
        let inner_elements = match_expr.get(1).map(|tt| match tt {
            TokenTree::Group(g) => g.stream(),
            _ => panic!("unexpected token after identifier"),
        });
        (match_ident, inner_elements)
    }

    trait MatchParsing {
        fn parse(match_expr: TokenStream) -> Self;
    }

    impl MatchParsing for ElementMatcher {
        fn parse(match_expr: TokenStream) -> ElementMatcher {
            let string = match_expr.to_string();
            if string == "_" {
                return ElementMatcher::Any;
            }
            if string == "x" {
                return ElementMatcher::X;
            }
            let (match_ident, inner_elements) = parse_element_matcher(match_expr);
            let match_ident_str = match_ident.to_string();
            let match_element =
                MatchElement::from_str(&match_ident_str).expect("unexpected element to match on");
            if let Some(inner) = inner_elements {
                match match_element {
                    MatchElement::Number => ElementMatcher::Number(NumberMatcher::parse(inner)),
                    MatchElement::Negate => ElementMatcher::Negate(Box::new(ElementMatcher::parse(inner))),
                    MatchElement::Plus => {
                        ElementMatcher::Plus(Box::new(SingleOrMultipleElementMatcher::parse(inner)))
                    },
                    MatchElement::Multiply => {
                        ElementMatcher::Multiply(Box::new(SingleOrMultipleElementMatcher::parse(inner)))
                    },
                    MatchElement::Pow => {
                        ElementMatcher::Pow(Box::new(SingleOrMultipleElementMatcher::parse(inner)))
                    },
                    MatchElement::Variable => ElementMatcher::Variable(StringMatcher::parse(inner)),
                    MatchElement::Function => {
                        let split = split_by_comma_2(inner);
                        if split.len() != 2 {
                            panic!("expected exactly two inner elements for function matcher");
                        }
                        ElementMatcher::Function(
                            StringMatcher::parse(split[0].clone()),
                            Box::new(SingleOrMultipleElementMatcher::parse(split[1].clone())),
                        )
                    },
                }
            } else {
                ElementMatcher::WithoutInner(match_element)
            }
        }
    }

    impl MatchParsing for NumberMatcher {
        fn parse(match_expr: TokenStream) -> NumberMatcher {
            if match_expr.to_string() == "x" {
                NumberMatcher::GetValue
            } else {
                NumberMatcher::CompareTo(match_expr)
            }
        }
    }

    impl MatchParsing for StringMatcher {
        fn parse(match_expr: TokenStream) -> Self {
            if match_expr.to_string() == "x" {
                StringMatcher::GetValue
            } else {
                StringMatcher::CompareTo(match_expr)
            }
        }
    }

    impl MatchParsing for SingleOrMultipleElementMatcher {
        fn parse(match_expr: TokenStream) -> Self {
            let split = split_by_comma_2(match_expr.clone());
            match split.len() {
                0 => panic!("no token provided for matching"),
                1 => {
                    let arr: Vec<_> = match_expr.clone().into_iter().collect();
                    if match_expr.to_string().ends_with("..") {
                        let tokens: TokenStream = arr[..(arr.len() - 2)].iter().cloned().collect();
                        SingleOrMultipleElementMatcher::Flatten(ElementMatcher::parse(tokens))
                    } else {
                        SingleOrMultipleElementMatcher::EachMatch(vec![ElementMatcher::parse(match_expr)])
                    }
                },
                2.. => SingleOrMultipleElementMatcher::EachMatch(
                    split.into_iter().map(|t| ElementMatcher::parse(t)).collect(),
                ),
            }
        }
    }

    fn outer(input: TokenStreamOld) -> TokenStreamOld {
        let new_stream = input.into();

        let MatchInput { formula, matcher } = MatchInput::parse(new_stream);
        let matcher = ElementMatcher::parse(matcher);
        let MatchOutput { tokens, var_count } = matcher.perform_match(formula);
        let mut tokens = quote! { (||#tokens)() };
        if var_count == 0 {
            tokens.append_all(quote! { .is_some() });
        }

        tokens.into()
    }
}
