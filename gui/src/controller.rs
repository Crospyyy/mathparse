use crate::logic::UiStateInfo;
use crate::ui::UiState;
use crate::{Window, WindowState};
use eframe::{App, CreationContext, Frame};
use egui::text::{CCursor, CCursorRange};
use egui::{
    CentralPanel, Color32, Context, Event, Key, Modifiers, Response, Shadow, Stroke, StrokeKind, Style,
    TextBuffer, TextEdit, Ui, ViewportCommand, Visuals,
};
use global_shortcuts::register_global_shortcut;
use library::{FormulaStore, Signature, Symbol, create_default_context, get_fun_name_end_of_string};
use regex::Regex;
use std::sync::LazyLock;

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
            let resp = ui.interact(ui.max_rect(), egui::Id::new("window-drag-bg"), egui::Sense::drag());
            if resp.dragged() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }

            let input = &mut vec![];
            self.show_top_input(ui, input, pressed_shortcut);
            ui.separator();
            self.show_all_defined_symbols(ui);
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
                UiStateInfo::TopInputSubmit => self.try_apply_calculation(),
                UiStateInfo::TopInputChanged | UiStateInfo::RoundingAccuracyChanged => {
                    self.update_calculation_result()
                },
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
            }
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
        if self.window_state.last_frame_had_focus && !has_focus {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        self.window_state.last_frame_had_focus = has_focus;

        if !self.window_state.centered {
            self.window_state.centered = try_center_window(ctx);
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
            self.ui_state.top_user_input = format!("{}({}){}", string_before, string_middle, string_after);
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
                    .map(|name| format!("Create new symbol '{}'", name.name())),
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
