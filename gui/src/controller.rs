use crate::logic::UiStateInfo;
use crate::ui::{HistoryEntry, HistoryEntryContent, Page, UiState};
use crate::{Window, WindowState};
use eframe::epaint::text::{LayoutJob, TextFormat, TextWrapMode, TextWrapping};
use eframe::epaint::{FontFamily, FontId};
use eframe::{App, CreationContext, Frame};
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditOutput;
use egui::{
    AtomExt, Button, CentralPanel, Color32, Context, DragValue, Event, Key, Label, Modifiers, PointerButton,
    Response, RichText, Shadow, Stroke, StrokeKind, Style, TextBuffer, TextEdit, Ui, ViewportCommand,
    Visuals, Widget, WidgetText,
};
use global_shortcuts::register_global_shortcut;
use library::{
    FormattedCalculationOutput, FormattingOptions, FormulaStore, RunResult, Signature, Symbol,
    create_default_context, get_fun_name_end_of_string, quick_match,
};
use regex::Regex;
use std::process::exit;
use std::sync::LazyLock;
use std::thread;

static REMOVE_OPERATIONS_BEFORE_CLOSING_BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([+\-*^]+)(\))").unwrap());

#[macro_export]
macro_rules! debug_print {
    ($($arg:tt)*) => {
        println!("[{}:{}:{}] {}",
            file!(),
            line!(),
            column!(),
            format_args!($($arg)*)
        )
    };
}

impl App for Window {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        let mut pressed_shortcut = false;
        self.handle_window_control(ctx, &mut pressed_shortcut);

        let frame = egui::containers::Frame::window(&Style::default());

        CentralPanel::default().frame(frame).show(ctx, |ui| {
            let resp =
                ui.interact(ui.max_rect(), egui::Id::new("window-drag-bg"), egui::Sense::click_and_drag());
            if resp.dragged_by(PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            } else {
                resp.context_menu(|ui| {
                    let mut extended_button =
                        |str: &str| Button::new(str).wrap_mode(TextWrapMode::Extend).ui(ui);

                    if extended_button("❌ Quit").clicked() {
                        exit(0);
                    }
                    if extended_button(if self.window_state.pinned {
                        "⏷ Unpin the window"
                    } else {
                        "📌 Pin the window"
                    })
                    .clicked()
                    {
                        self.window_state.pinned ^= true;
                        if !self.window_state.pinned {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                    }
                });
            }

            let input = &mut vec![];
            self.show_top_input(ui, input, pressed_shortcut);
            self.show_tab_selector(ui, input);
            match self.ui_state.selected_page {
                Page::History => self.show_history(ui),
                Page::DefinedSymbols => self.show_all_defined_symbols(ui),
            }
            self.handle_ui_input(input);
        });
    }

    fn clear_color(&self, _visuals: &Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}

pub fn try_center_window(ctx: &Context) -> bool {
    let (monitor_opt, win_size_opt) =
        ctx.input(|i| (i.viewport().monitor_size, i.viewport().outer_rect.map(|r| r.size())));

    if let (Some(monitor), Some(win_size)) = (monitor_opt, win_size_opt) {
        let pos = (monitor - win_size) / 2.0;
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos.to_pos2()));
        debug_print!("Center window at: {:?}", pos);
        return true;
    }
    false
}

impl Window {
    pub(crate) fn new(_cc: &CreationContext) -> Self {
        let ppp = _cc.egui_ctx.pixels_per_point();
        _cc.egui_ctx.set_pixels_per_point(ppp * 1.2);
        let mut store = FormulaStore::new_empty();
        store.define_default_symbols().unwrap();
        store.add_symbol_from_string("speed_of_sound_mps = 343", false).unwrap();
        store.add_symbol_from_string("speed_of_light_mps = 299_792_458", false).unwrap();
        store.add_symbol_from_string("kw_to_ps = 1.35962", false).unwrap();
        store.add_symbol_from_string("km_to_miles = 0.6214", false).unwrap();
        store.add_symbol_from_string("liter_to_gallons = 0.264172", false).unwrap();
        store.add_symbol_from_string("joule_to_wh = 1/3600", false).unwrap();
        store.add_symbol_from_string("water_heat_capacity_j_per_g = 4.184", false).unwrap();
        let mut window =
            Self { formula_store: store, ui_state: UiState::new(), window_state: WindowState::new() };
        window.update_all_symbol_strings();

        let context = _cc.egui_ctx.clone();
        context.send_viewport_cmd(ViewportCommand::Minimized(true));
        let req_focus = window.window_state.request_focus.clone();
        register_global_shortcut(global_shortcuts::Modifiers::ALT, global_shortcuts::Key::Space, move || {
            context.send_viewport_cmd(ViewportCommand::Minimized(false));
            context.send_viewport_cmd(ViewportCommand::Focus);
            req_focus.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        window
    }

    pub fn handle_ui_input(&mut self, input: &mut Vec<UiStateInfo>) {
        for info in input.drain(..) {
            match info {
                UiStateInfo::TopInputChanged | UiStateInfo::RoundingAccuracyChanged => {
                    self.update_calculation_result()
                },
                UiStateInfo::TopInputSubmit => self.try_apply_calculation(),
                UiStateInfo::RequestAutocompletion { cursor_pos, input_term, complete_to, response } => {
                    let insert_brackets = self.formula_store.get_symbol(&complete_to).is_some_and(|s| {
                        matches!(
                            s.signature(),
                            Signature::Function(..) | Signature::FunctionNOrMoreParams(..)
                        )
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
                UiStateInfo::SelectPage(page) => {
                    self.ui_state.selected_page = page;
                },
            }
        }
    }

    pub(crate) fn show_top_input(
        &mut self, ui: &mut Ui, input: &mut Vec<UiStateInfo>, pressed_shortcut: bool,
    ) {
        let text_before = self.ui_state.top_user_input.clone();
        self.preprocess_user_input(ui);

        let output = self.show_top_input_textedit(ui);
        let response = output.response.clone();
        if pressed_shortcut {
            self.select_all_in_textedit(&response);
        }
        self.input_post_process(ui);

        if response.has_focus() {
            let autocompletion_visible = self.show_autocompletion(ui, &response, input);
            if autocompletion_visible {
                self.show_inline_result(ui, &output);
            }
        }

        if text_before != self.ui_state.top_user_input {
            input.push(UiStateInfo::TopInputChanged);
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            response.request_focus();
            input.push(UiStateInfo::TopInputSubmit);
        }

        self.handle_ui_input(input);

        self.show_result_label(ui, input);
    }

    fn select_all_in_textedit(&mut self, response: &Response) {
        if let Some(mut state) = TextEdit::load_state(&response.ctx, response.id) {
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(0),
                CCursor::new(self.ui_state.top_user_input.len()),
            )));
            state.store(&response.ctx, response.id);
        }
    }

    pub(super) fn handle_window_control(&mut self, ctx: &Context, pressed_shortcut: &mut bool) {
        let requested_focus = self.window_state.request_focus.load(std::sync::atomic::Ordering::Relaxed);
        if requested_focus {
            self.window_state.request_focus.store(false, std::sync::atomic::Ordering::Relaxed);
            ctx.memory_mut(|mem| mem.request_focus(self.ui_state.top_user_input_id));
            *pressed_shortcut = true;
        }
        let has_focus = ctx.input(|ip| ip.raw.focused);
        if !self.window_state.pinned && self.window_state.last_frame_had_focus && !has_focus {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        self.window_state.last_frame_had_focus = has_focus;

        if !self.window_state.centered {
            self.window_state.centered = try_center_window(ctx);
        }
    }

    pub(super) fn preprocess_user_input(&mut self, ui: &mut Ui) {
        let Some(mut typed_brackets) = ui.input_mut(|ip| {
            ip.events.retain(|e| match e {
                Event::Text(text) => text.chars().all(|c| c.is_ascii()),
                _ => true,
            });
            let text_bracket_check = |t: &&String| t.ends_with("(");
            if let Some(bracket_string) = ip
                .events
                .iter()
                .flat_map(|e| quick_match!(e, Event::Text(t)=>t))
                .find(text_bracket_check)
                .cloned()
            {
                ip.consume_key(Modifiers::NONE, Key::OpenBracket);
                ip.consume_key(Modifiers::SHIFT, Key::Num8);
                ip.events.retain(|e| match e {
                    Event::Text(t) => !text_bracket_check(&t),
                    _ => true,
                });
                Some(bracket_string)
            } else {
                None
            }
        }) else {
            return;
        };

        let Some(mut state) = TextEdit::load_state(ui.ctx(), self.ui_state.top_user_input_id) else {
            return;
        };
        let Some(cursors) = state.cursor.char_range() else {
            return;
        };
        if let Some(cursor_pos) = cursors.single().map(|c| c.index) {
            let move_cursor = typed_brackets.len();
            if cursor_pos == self.ui_state.top_user_input.len() {
                typed_brackets += ")";
            }
            self.ui_state.top_user_input.insert_str(cursor_pos, &typed_brackets);
            state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(cursor_pos + move_cursor))));
        } else {
            let [min, max] = cursors.sorted_cursors().map(|c| c.index);
            let string_before = &self.ui_state.top_user_input[..min];
            let string_middle = &self.ui_state.top_user_input[min..max];
            let string_after = &self.ui_state.top_user_input[max..];
            self.ui_state.top_user_input = format!("{}({}){}", string_before, string_middle, string_after);
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(cursors.primary.index + 1),
                CCursor::new(cursors.secondary.index + 1),
            )));
        }
        state.store(ui.ctx(), self.ui_state.top_user_input_id);
    }

    pub(super) fn input_post_process(&mut self, ui: &mut Ui) {
        self.ui_state.top_user_input.retain(|c| c.is_ascii());
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
        } else {
            let result = self.formula_store.run(input, true);
            Some(match result {
                RunResult::ParseFailed(s) => Err(format!("Parse Error: {}", s)),
                RunResult::CalculationFailed(s) => Err(format!("Calculation Error: {}", s)),
                RunResult::FailedToAddSymbol(s) => Err(format!("Error adding symbol: {}", s)),
                RunResult::CalculationResult(r) => {
                    let formatting_options =
                        FormattingOptions::default().with_rounding(self.ui_state.output_digits);
                    let output = r.to_string_detailed(formatting_options);
                    Ok(match output {
                        FormattedCalculationOutput::Exact { result, has_rounded } => {
                            if has_rounded {
                                format!("= {} (rounded)", result)
                            } else {
                                format!("= {}", result)
                            }
                        },
                        FormattedCalculationOutput::ApproximationChecked { result, precision_bits } => {
                            format!("≈ {} ({} bit precision)", result, precision_bits)
                        },
                        FormattedCalculationOutput::ApproximationReachedLimit { result } => {
                            format!("≈ {} (reached precision limit)", result)
                        },
                    })
                },
                RunResult::AddedSymbol(s) => {
                    Ok(format!("Create new symbol: {}", s.symbol().get_full_string(s.name(), ctx)))
                },
            })
        }
    }

    fn try_apply_calculation(&mut self) {
        let input = self.get_processed_input();
        if input.is_empty() {
            return;
        }
        if !input.contains("=") {
            if let Some(Ok(result)) = &self.ui_state.calculation_result {
                if self
                    .ui_state
                    .history
                    .last()
                    .is_some_and(|entry| matches!(
                        &entry.content,
                        HistoryEntryContent::Calculation(e_input, e_result) if e_input == &input && e_result == result)
                    )
                {
                    return;
                }
                self.ui_state.history.push(HistoryEntry::new_calculation(
                    self.ui_state.top_user_input.clone(),
                    result.clone(),
                    chrono::Local::now().format("%H:%M").to_string(),
                ))
            }
            return;
        }
        let result = self.formula_store.add_symbol_from_string(&input, false);
        if let Ok(r) = result {
            self.ui_state.history.push(HistoryEntry::new_symbol_definition(
                format!("⛃ {}", r.symbol().get_full_string(r.name(), &mut create_default_context())),
                chrono::Local::now().format("%H:%M").to_string(),
            ));
            self.ui_state.top_user_input.clear();
            self.update_calculation_result();
            self.update_all_symbol_strings();
        }
    }

    fn update_all_symbol_strings(&mut self) {
        let elements = self.formula_store.get_symbols_sorted();
        let ctx = &mut create_default_context();
        self.ui_state.all_symbol_strings =
            elements.iter().map(|(name, symbol)| symbol.get_full_string(name, ctx)).collect();
    }

    pub fn get_autocompletion_result(
        &self, input_string: &str, cursor_pos: usize,
    ) -> Option<AutocompletionResult> {
        let input_symbol_name = get_fun_name_end_of_string(&input_string.char_range(0..cursor_pos), false);
        if input_symbol_name.is_empty() {
            return None;
        }
        let compatible_symbols = self
            .formula_store
            .get_symbols_sorted()
            .into_iter()
            .filter(|(name, _)| name.starts_with(&input_symbol_name))
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
    pub possible_symbols: Vec<(&'a String, &'a Symbol)>,
    pub longest_common_start: String,
}

pub fn set_cursor_pos(response: &Response, cursor_pos: usize) {
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
