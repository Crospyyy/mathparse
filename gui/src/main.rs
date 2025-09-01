use crate::ui::UiState;
use library::FormulaStore;

pub fn main() {
    use eframe::NativeOptions;
    eframe::run_native("Quick Mafs", NativeOptions::default(), Box::new(|cc| Ok(Box::new(Window::new(cc)))))
        .expect("panic message");
}

pub struct Window {
    formula_store: FormulaStore,
    ui_state: UiState,
}

mod ui {
    use crate::Window;
    use crate::controller::get_cursor_pos;
    use crate::logic::UiStateInfo;
    use eframe::epaint::text::{LayoutJob, TextFormat, TextWrapping};
    use eframe::epaint::{Color32, FontFamily, FontId};
    use eframe::{App, Frame};
    use egui::{
        CentralPanel, Context, DragValue, FontSelection, Id, Key, Label, Response, RichText, ScrollArea,
        TextEdit, Ui, Widget,
    };
    use library::FormattingOptions;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    pub(super) struct UiState {
        pub(super) request_focus: Arc<AtomicBool>,
        pub(super) top_user_input: String,
        pub(super) top_user_input_id: Id,
        pub(super) calculation_result: Option<Result<String, String>>,
        pub(super) output_digits: usize,
        pub all_symbol_strings: Vec<String>,
    }

    impl UiState {
        pub(super) fn new() -> Self {
            Self {
                request_focus: Arc::new(AtomicBool::new(false)),
                top_user_input: "".to_string(),
                top_user_input_id: "Formula Input".into(),
                calculation_result: None,
                output_digits: FormattingOptions::default().round_to_decimals,
                all_symbol_strings: Vec::new(),
            }
        }
    }

    impl App for Window {
        fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
            CentralPanel::default().show(ctx, |ui| {
                let input = &mut vec![];
                self.show_top_input(ui, input);
                ui.separator();
                self.show_all_defined_symbols(ui);
                self.handle_ui_input(input);
            });
        }
    }

    impl Window {
        pub(crate) fn show_top_input(&mut self, ui: &mut Ui, input: &mut Vec<UiStateInfo>) {
            if self.ui_state.request_focus.load(std::sync::atomic::Ordering::Relaxed) {
                self.ui_state.request_focus.store(false, std::sync::atomic::Ordering::Relaxed);
                ui.ctx().memory_mut(|mem| mem.request_focus(self.ui_state.top_user_input_id))
            }
            self.handle_bracket_input(ui);

            let text_edit = TextEdit::singleline(&mut self.ui_state.top_user_input)
                .id(self.ui_state.top_user_input_id)
                .font(FontSelection::FontId(FontId::new(20.0, FontFamily::Proportional)))
                .lock_focus(true);

            let response = ui.add_sized([ui.available_width(), 20.0], text_edit);
            self.input_post_process(ui);

            if response.has_focus() {
                self.show_autocompletion(ui, &response, input);
            }
            self.handle_ui_input(input);

            if response.changed() {
                input.push(UiStateInfo::TopInputChanged);
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                response.request_focus();
                input.push(UiStateInfo::TopInputSubmit);
            }

            self.handle_ui_input(input);

            // todo only show an error icon and display the error message in a tooltip
            let mut job = LayoutJob::default();
            let text = match &self.ui_state.calculation_result {
                Some(Ok(result)) => result,
                Some(Err(_)) => "=  !",
                None => "",
            };

            let mut format =
                TextFormat { font_id: FontId::new(20.0, FontFamily::Proportional), ..Default::default() };
            if self.ui_state.calculation_result.as_ref().is_some_and(|r| r.is_err()) {
                format.color = Color32::ORANGE.gamma_multiply(0.7);
            }
            job.append(text, 0.0, format);
            job.wrap = TextWrapping {
                max_width: ui.available_width(),
                max_rows: usize::MAX,
                break_anywhere: true,
                overflow_character: None,
            };
            let error_string = self.ui_state.calculation_result.as_ref().and_then(|r| r.as_ref().err());

            let result_resp = Label::new(job).selectable(error_string.is_none()).ui(ui);
            if let Some(err) = error_string {
                result_resp.on_hover_text(err);
            }
            ui.horizontal(|ui| {
                ui.label("Round Digits:");
                let drag_val_resp = DragValue::new(&mut self.ui_state.output_digits).range(1..=100).ui(ui);
                if drag_val_resp.changed() {
                    input.push(UiStateInfo::RoundingAccuracyChanged);
                }
            });
        }

        fn show_autocompletion(&mut self, ui: &mut Ui, response: &Response, input: &mut Vec<UiStateInfo>) {
            let Some(cursor_pos) = get_cursor_pos(&response) else { return };
            let Some(autocompletion) =
                self.get_autocompletion_result(&self.ui_state.top_user_input, cursor_pos)
            else {
                return;
            };

            let already_input = autocompletion.input_term.len();

            response.show_tooltip_ui(|ui| {
                for symbol in autocompletion.possible_symbols {
                    let mut job = LayoutJob::default();
                    job.append(
                        &autocompletion.input_term,
                        0.0,
                        TextFormat::simple(FontId::default(), ui.visuals().text_color()),
                    );
                    job.append(
                        &symbol.get_signature_string()[already_input..],
                        0.0,
                        TextFormat::simple(FontId::default(), ui.visuals().weak_text_color()),
                    );
                    Label::new(job).extend().ui(ui);
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                input.push(UiStateInfo::RequestAutocompletion {
                    cursor_pos,
                    input_term: autocompletion.input_term,
                    complete_to: autocompletion.longest_common_start,
                    response: response.clone(),
                });
            }
        }

        fn show_all_defined_symbols(&mut self, ui: &mut Ui) {
            // todo align all symbols to the `=` sign
            let area = ScrollArea::vertical().auto_shrink(false);
            area.show(ui, |ui| {
                for text in &self.ui_state.all_symbol_strings {
                    ui.label(RichText::new(text).size(17.0));
                }
            });
        }
    }
}

mod logic {
    use crate::Window;
    use egui::Response;
    use library::{Benchmark, FormattingOptions, NumberContext, NumberString, benchmark};

    impl Window {
        pub(crate) fn get_result_of_possible_symbol_declaration(
            &mut self, input: &str,
        ) -> Result<String, String> {
            self.formula_store.add_symbol_from_string(input, true)
        }

        pub(crate) fn evaluate_formula(
            &mut self, ctx: &mut NumberContext, input: &String,
        ) -> Result<String, String> {
            let mut benchmark = Benchmark::new();
            self.formula_store
                .eval_with_benchmark(&input, ctx, &mut benchmark)
                .map(|result| {
                    benchmark.print_times();
                    let mut b = Benchmark::new();
                    let result_str = benchmark!(
                        b,
                        result.to_string_detailed(
                            FormattingOptions::default().with_rounding(self.ui_state.output_digits),
                            ctx,
                        ),
                        "Formatting number"
                    );
                    b.print_times();

                    match result_str {
                        NumberString::Imprecise(str) => format!("~= {}", str),
                        NumberString::Precise { string, is_rounded } => {
                            if is_rounded {
                                format!("= {} (rounded)", string)
                            } else {
                                format!("= {}", string)
                            }
                        },
                    }
                })
                .map_err(|s| format!("Error: {}", s))
        }
    }

    pub enum UiStateInfo {
        TopInputChanged,
        TopInputSubmit,
        RoundingAccuracyChanged,
        RequestAutocompletion {
            cursor_pos: usize,
            input_term: String,
            complete_to: String,
            response: Response,
        },
    }
}

mod controller {
    use crate::Window;
    use crate::logic::UiStateInfo;
    use crate::ui::UiState;
    use eframe::CreationContext;
    use egui::text::{CCursor, CCursorRange};
    use egui::{Event, Key, Modifiers, Response, TextBuffer, TextEdit, Ui, ViewportCommand};
    use global_shortcuts::register_global_shortcut;
    use library::{FormulaStore, Signature, Symbol, create_default_context, get_fun_name_end_of_string};
    use regex::Regex;
    use std::sync::LazyLock;

    static REMOVE_OPERATIONS_BEFORE_CLOSING_BRACKETS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"([+\-*^]+)(\))").unwrap());

    impl Window {
        pub(crate) fn new(_cc: &CreationContext) -> Self {
            let mut store = FormulaStore::new_empty();
            store.define_default_symbols().unwrap();
            store.add_symbol_from_string("speed_of_sound_mps = 343", false).unwrap();
            store.add_symbol_from_string("speed_of_light_mps = 299_792_458", false).unwrap();
            store.add_symbol_from_string("kw_to_ps = 1.35962", false).unwrap();
            store.add_symbol_from_string("km_to_miles = 0.6214", false).unwrap();
            store.add_symbol_from_string("liter_to_gallons = 0.264172", false).unwrap();
            store.add_symbol_from_string("joule_to_wh = 1/3600", false).unwrap();
            store.add_symbol_from_string("water_heat_capacity_j_per_g = 4.184", false).unwrap();
            let mut window = Self { formula_store: store, ui_state: UiState::new() };
            window.update_all_symbol_strings();
            let context = _cc.egui_ctx.clone();
            let req_focus = window.ui_state.request_focus.clone();
            register_global_shortcut(
                global_shortcuts::Modifiers::ALT,
                global_shortcuts::Key::Space,
                move || {
                    context.send_viewport_cmd(ViewportCommand::Minimized(false));
                    context.send_viewport_cmd(ViewportCommand::Focus);
                    req_focus.store(true, std::sync::atomic::Ordering::Relaxed);
                },
            );
            window
        }

        pub fn handle_ui_input(&mut self, input: &mut Vec<UiStateInfo>) {
            for info in input.drain(..) {
                match info {
                    UiStateInfo::TopInputSubmit => self.try_apply_calculation(),
                    UiStateInfo::TopInputChanged | UiStateInfo::RoundingAccuracyChanged => {
                        self.update_calculation_result()
                    },
                    UiStateInfo::RequestAutocompletion { cursor_pos, input_term, complete_to, response } => {
                        let insert_brackets =
                            self.formula_store.get_signature(&complete_to).is_some_and(|s| {
                                matches!(s, Signature::Function(..) | Signature::FunctionNOrMoreParams(..))
                            });

                        let mut insert = complete_to[input_term.len()..].to_string();
                        let mut new_cursor_pos = cursor_pos - input_term.len() + complete_to.len();

                        if insert_brackets {
                            if self.ui_state.top_user_input.chars().nth(cursor_pos) != Some('(') {
                                insert.push_str("()");
                            }
                            new_cursor_pos += 1;
                        }

                        self.ui_state.top_user_input.insert_str(cursor_pos, &insert);
                        set_cursor_pos(&response, new_cursor_pos);
                        self.update_calculation_result();
                    },
                }
            }
        }

        pub(super) fn handle_bracket_input(&mut self, ui: &mut Ui) {
            let typed_brackets = ui.input_mut(|ip| {
                let typed_bracket = ip.consume_key(Modifiers::NONE, Key::OpenBracket)
                    | ip.consume_key(Modifiers::SHIFT, Key::Num8);
                ip.events.retain(|e| e != &Event::Text("(".to_owned()));
                typed_bracket
            });
            if !typed_brackets {
                return;
            }
            let Some(mut state) = TextEdit::load_state(ui.ctx(), self.ui_state.top_user_input_id) else {
                return;
            };
            let Some(cursors) = state.cursor.char_range() else {
                return;
            };
            if let Some(cursor_pos) = cursors.single().map(|c| c.index) {
                self.ui_state.top_user_input.insert_str(
                    cursor_pos,
                    if cursor_pos == self.ui_state.top_user_input.len() { "()" } else { "(" },
                );
                state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(cursor_pos + 1))));
            } else {
                let [min, max] = cursors.sorted_cursors().map(|c| c.index);
                let string_before = &self.ui_state.top_user_input[..min];
                let string_middle = &self.ui_state.top_user_input[min..max];
                let string_after = &self.ui_state.top_user_input[max..];
                self.ui_state.top_user_input =
                    format!("{}({}){}", string_before, string_middle, string_after);
                state.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::new(cursors.primary.index + 1),
                    CCursor::new(cursors.secondary.index + 1),
                )));
            }
            state.store(ui.ctx(), self.ui_state.top_user_input_id);
        }

        pub(super) fn input_post_process(&mut self, ui: &mut Ui) {
            if !ui.input(|ip| ip.events.iter().any(|e| matches!(e, Event::Text(_)))) {
                return;
            };
            let Some(mut state) = TextEdit::load_state(ui.ctx(), self.ui_state.top_user_input_id) else {
                return;
            };
            let Some(cursor_pos) = state.cursor.char_range().map(|c| c.sorted_cursors()[1].index) else {
                return;
            };
            let string =
                get_fun_name_end_of_string(&self.ui_state.top_user_input.char_range(0..cursor_pos), true);
            if string.is_empty() || string.chars().nth(0).is_some_and(|c| !c.is_digit(10)) {
                return;
            }
            let Some(first_char_pos) = string.chars().position(|c| !c.is_digit(10)) else {
                return;
            };
            let pos_before = cursor_pos - string.len();
            let insert_pos = pos_before + first_char_pos;
            self.ui_state.top_user_input.insert_str(insert_pos, "*");
            let mut add_move_cursor_right = 1;

            if pos_before > 0 {
                let mut first_num_char = pos_before;
                loop {
                    if first_num_char == 0 {
                        break;
                    }
                    let new_first = first_num_char - 1;
                    if matches!(self.ui_state.top_user_input.chars().nth(new_first), Some('0'..='9' | '.')) {
                        first_num_char = new_first;
                    } else {
                        break;
                    }
                }
                if first_num_char != 0
                    && matches!(self.ui_state.top_user_input.chars().nth(first_num_char - 1), Some('^' | '/'))
                {
                    self.ui_state.top_user_input.insert(cursor_pos + 1, ')');
                    self.ui_state.top_user_input.insert(first_num_char, '(');
                    add_move_cursor_right += 1;
                }
            }
            state
                .cursor
                .set_char_range(Some(CCursorRange::one(CCursor::new(cursor_pos + add_move_cursor_right))));
            state.store(ui.ctx(), self.ui_state.top_user_input_id);
        }

        fn get_processed_input(&self) -> String {
            let mut string = self.ui_state.top_user_input.trim().to_string();

            while matches!(string.chars().last(), Some('=' | '-' | '+' | '*' | '/' | '^')) {
                string.pop();
            }
            string = REMOVE_OPERATIONS_BEFORE_CLOSING_BRACKETS.replace_all(&string, "$2").to_string();

            string
        }

        fn update_calculation_result(&mut self) {
            let ctx = &mut create_default_context();
            let input = self.get_processed_input();

            self.ui_state.calculation_result = if input.is_empty() {
                None
            } else if input.contains("=") {
                Some(
                    self.get_result_of_possible_symbol_declaration(&input)
                        .map(|name| format!("Create new symbol '{}'", name)),
                )
            } else {
                Some(self.evaluate_formula(ctx, &input))
            }
        }

        fn try_apply_calculation(&mut self) {
            let input = self.get_processed_input();
            if input.is_empty() || !input.contains("=") {
                return;
            }
            if self.formula_store.add_symbol_from_string(&input, false).is_ok() {
                self.ui_state.top_user_input.clear();
                self.update_calculation_result();
                self.update_all_symbol_strings();
            }
        }

        fn update_all_symbol_strings(&mut self) {
            let mut elements = self.formula_store.get_symbols();
            elements.sort_by_cached_key(|e| e.name().to_string());
            let ctx = &mut create_default_context();
            self.ui_state.all_symbol_strings =
                elements.iter().map(|symbol| symbol.get_full_string(ctx)).collect();
        }

        pub fn get_autocompletion_result(
            &self, input_string: &str, cursor_pos: usize,
        ) -> Option<AutocompletionResult> {
            let input_symbol_name =
                get_fun_name_end_of_string(&input_string.char_range(0..cursor_pos), false);
            if input_symbol_name.is_empty() {
                return None;
            }
            let compatible_symbols = self
                .formula_store
                .get_symbols()
                .into_iter()
                .filter(|symbol| symbol.name().starts_with(&input_symbol_name))
                .collect::<Vec<_>>();

            if compatible_symbols.is_empty() {
                return None;
            }
            let longest_common_start = determine_longest_common_start(&compatible_symbols);
            Some(AutocompletionResult {
                input_term: input_symbol_name,
                possible_symbols: compatible_symbols,
                longest_common_start,
            })
        }
    }

    pub struct AutocompletionResult<'a> {
        pub input_term: String,
        pub possible_symbols: Vec<Symbol<'a>>,
        pub longest_common_start: String,
    }

    fn set_cursor_pos(response: &Response, cursor_pos: usize) {
        if let Some(mut state) = TextEdit::load_state(&response.ctx, response.id) {
            state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(cursor_pos))));
            state.store(&response.ctx, response.id);
        }
    }

    pub fn get_cursor_pos(response: &Response) -> Option<usize> {
        if let Some(state) = TextEdit::load_state(&response.ctx, response.id) {
            state.cursor.char_range().and_then(|c| c.single()).map(|c| c.index)
        } else {
            None
        }
    }

    pub fn determine_longest_common_start(names: &[Symbol]) -> String {
        if names.is_empty() {
            return String::new();
        }
        let common_start = names[0].name().to_string();
        let mut longest_common = common_start.len();
        for name in names.iter().map(|s| s.name()).skip(1) {
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
}
