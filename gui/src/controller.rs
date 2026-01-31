use crate::logic::{Backend, UiInteraction};
use crate::ui::UiState;
use crate::ui::bottom_panel::{HistoryEntry, HistoryEntryContent, Page};
use crate::window_control::{WindowState, switch_visibility};
use crate::{Window, logic};
use eframe::{App, CreationContext, Frame};
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditOutput;
use egui::{
	Align, Button, CentralPanel, Color32, Context, CursorIcon, Event, Id, Key, Layout, Modifiers, OpenUrl,
	RawInput, Response, RichText, Shadow, Style, TextBuffer, TextEdit, TextWrapMode, Ui, Visuals, Widget,
};
use library::{
	FormulaStore, RunResult, RunSuccess, Signature, Symbol, convert_from_latex_if_needed, debug_print,
	get_fun_name_end_of_string, only_in_debug, quick_match,
};
use regex::Regex;
use std::ops::Not;
use std::process::exit;
use std::sync::LazyLock;
use std::sync::mpsc::{Receiver, Sender, channel};

static REMOVE_OPERATIONS_BEFORE_CLOSING_BRACKETS: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"([+\-*^]+)(\))").unwrap());

impl App for Window {
	fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
		let mut pressed_shortcut = false;
		self.handle_window_control(ctx, &mut pressed_shortcut);

		let frame = if self.window_state.is_pinned() {
			egui::containers::Frame::central_panel(&Style::default())
		} else {
			let shadow = Shadow {
				color: Color32::BLACK.gamma_multiply(0.1),
				blur: 14,
				spread: 7,
				..Default::default()
			};
			egui::containers::Frame::window(&Style::default())
				.corner_radius(10.0)
				.inner_margin(10.0)
				.outer_margin(15.0)
				.shadow(shadow)
		};

		CentralPanel::default().frame(frame).show(ctx, |ui| {
			self.window_background_logic(ctx, ui);
			self.show_top_input(ui, pressed_shortcut);
			ui.add_space(7.5);
			self.ui_state.show_tab_selector(ui);
			ui.separator();
			match self.ui_state.bottom_panel.selected_page {
				Page::History => self.ui_state.show_history(ui),
				Page::DefinedSymbols => self.ui_state.show_all_defined_symbols(ui),
			}
			if ui.available_height() > 20.0 {
				ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
					if Button::new(RichText::new(" Crospy  ").weak().size(10.0))
						.frame(false)
						.ui(ui)
						.on_hover_cursor(CursorIcon::PointingHand)
						.clicked()
					{
						ctx.open_url(OpenUrl::new_tab("https://github.com/Crospyyy"));
					}
				});
			}
			self.handle_ui_input();
		});
	}

	fn clear_color(&self, _visuals: &Visuals) -> [f32; 4] {
		egui::Rgba::TRANSPARENT.to_array()
	}

	fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut RawInput) {
		self.raw_input_hook_inner(ctx, raw_input)
	}
}

impl Window {
	pub(crate) fn new(cc: &CreationContext) -> Self {
		let ctx = cc.egui_ctx.clone();

		ctx.set_pixels_per_point(ctx.pixels_per_point() * 1.2);

		let backend = Backend::new();

		let mut ui_state = UiState::empty();
		ui_state.update_all_symbol_strings(backend.formula_store());

		let window_state = WindowState::new();
		window_state.start_shortcut_listener(&cc.egui_ctx);

		let window = Self { backend, ui_state, window_state };

		switch_visibility(&ctx, false, window.get_last_window_size());
		window
	}

	pub fn handle_ui_input(&mut self) {
		while let Ok(info) = self.ui_state.interaction.0.try_recv() {
			match info {
				UiInteraction::TopInputChanged | UiInteraction::RoundingAccuracyChanged => {
					self.update_calculation_result()
				},
				UiInteraction::TopInputSubmit => self.try_apply_calculation(),
				UiInteraction::RequestAutocompletion { cursor_pos, input_term, complete_to, response } => {
					let insert_brackets = self.backend.get_symbol(&complete_to).is_some_and(|s| {
						matches!(
							s.signature(),
							Signature::Function(..) | Signature::FunctionNOrMoreParams(..)
						)
					});

					let mut insert = complete_to[input_term.len()..].to_string();
					let mut new_cursor_pos = cursor_pos - input_term.len() + complete_to.len();

					if insert_brackets {
						if self
							.ui_state
							.calculation_panel
							.calculation_input
							.top_user_input
							.chars()
							.nth(cursor_pos) != Some('(')
						{
							insert.push_str("()");
						}
						new_cursor_pos += 1;
					}

					self.ui_state
						.calculation_panel
						.calculation_input
						.top_user_input
						.insert_str(cursor_pos, &insert);
					set_cursor_pos(&response, new_cursor_pos);
					self.update_calculation_result();
				},
				UiInteraction::SelectPage(page) => {
					self.ui_state.bottom_panel.selected_page = page;
				},
				UiInteraction::ClearCustomSymbols => {
					if !self.backend.has_custom_symbols() {
						return;
					}
					self.backend.clear_custom_symbols();
					self.ui_state
						.bottom_panel
						.history
						.push(HistoryEntry::new(HistoryEntryContent::ClearedSymbols));
					self.ui_state.update_all_symbol_strings(self.backend.formula_store());
					self.update_calculation_result();
				},
				UiInteraction::ClearHistory => {
					self.ui_state.bottom_panel.history.clear();
				},
			}
		}
	}

	pub(crate) fn show_top_input(&mut self, ui: &mut Ui, pressed_shortcut: bool) {
		let text_before = self.ui_state.calculation_panel.calculation_input.top_user_input.clone();
		self.preprocess_user_input(ui);

		let output = self.ui_state.show_top_input_textedit(ui);
		let response = output.response.clone();
		if pressed_shortcut {
			self.select_all_in_textedit(&response);
		}

		self.input_post_process(ui);

		if response.has_focus() {
			self.ui_state.show_brackets_highlighting(ui, &output);
			let autocompletion_visible =
				self.ui_state.show_autocompletion(ui, &response, self.backend.formula_store());
			if autocompletion_visible
				|| self.ui_state.calculation_panel.calculation_input.top_user_input.contains('=')
			{
				self.ui_state.show_inline_result(ui, &output);
			}
		}

		if text_before != self.ui_state.calculation_panel.calculation_input.top_user_input {
			self.ui_state.interaction_sender.send(UiInteraction::TopInputChanged);
		}
		if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
			response.request_focus();
			self.ui_state.interaction_sender.send(UiInteraction::TopInputSubmit);
		}

		self.handle_ui_input();
		
		self.ui_state.show_main_result(ui);
	}

	fn select_all_in_textedit(&mut self, response: &Response) {
		if let Some(mut state) = TextEdit::load_state(&response.ctx, response.id) {
			state.cursor.set_char_range(Some(CCursorRange::two(
				CCursor::new(0),
				CCursor::new(self.ui_state.calculation_panel.calculation_input.top_user_input.len()),
			)));
			state.store(&response.ctx, response.id);
		}
	}

	pub(crate) fn request_top_input_focus(&mut self, ctx: &Context) {
		ctx.memory_mut(|mem| {
			mem.request_focus(self.ui_state.calculation_panel.calculation_input.top_user_input_id)
		});
	}

	pub(super) fn preprocess_user_input(&mut self, ui: &mut Ui) {
		self.preprocess_brackets(ui);
		self.preprocess_paste(ui);
	}

	pub(super) fn input_post_process(&mut self, ui: &mut Ui) {
		self.ui_state.calculation_panel.calculation_input.top_user_input.retain(|c| c.is_ascii());
		if !ui.input(|ip| ip.events.iter().any(|e| matches!(e, Event::Text(_)))) {
			return;
		};
		let Some(mut state) = TextEdit::load_state(
			ui.ctx(),
			self.ui_state.calculation_panel.calculation_input.top_user_input_id,
		) else {
			return;
		};
		let Some(cursor_pos) = state.cursor.char_range().map(|c| c.sorted_cursors()[1].index) else {
			return;
		};
		let string = get_fun_name_end_of_string(
			self.ui_state.calculation_panel.calculation_input.top_user_input.char_range(0..cursor_pos),
			true,
		);
		if string.is_empty() || string.chars().nth(0).is_some_and(|c| !c.is_ascii_digit()) {
			return;
		}
		let Some(first_char_pos) = string.chars().position(|c| !c.is_ascii_digit()) else {
			return;
		};
		let pos_before = cursor_pos - string.len();
		let insert_pos = pos_before + first_char_pos;
		self.ui_state.calculation_panel.calculation_input.top_user_input.insert(insert_pos, '*');
		let mut add_move_cursor_right = 1;

		if pos_before > 0 {
			let mut first_num_char = pos_before;
			loop {
				if first_num_char == 0 {
					break;
				}
				let new_first = first_num_char - 1;
				if matches!(
					self.ui_state.calculation_panel.calculation_input.top_user_input.chars().nth(new_first),
					Some('0'..='9' | '.')
				) {
					first_num_char = new_first;
				} else {
					break;
				}
			}
			if first_num_char != 0
				&& matches!(
					self.ui_state
						.calculation_panel
						.calculation_input
						.top_user_input
						.chars()
						.nth(first_num_char - 1),
					Some('^' | '/')
				) {
				self.ui_state.calculation_panel.calculation_input.top_user_input.insert(cursor_pos + 1, ')');
				self.ui_state.calculation_panel.calculation_input.top_user_input.insert(first_num_char, '(');
				add_move_cursor_right += 1;
			}
		}
		state
			.cursor
			.set_char_range(Some(CCursorRange::one(CCursor::new(cursor_pos + add_move_cursor_right))));
		state.store(ui.ctx(), self.ui_state.calculation_panel.calculation_input.top_user_input_id);
	}

	fn get_processed_input(&self) -> String {
		let mut string = self.ui_state.calculation_panel.calculation_input.top_user_input.trim().to_string();
		string.retain(|c| c != ' ');

		while matches!(string.chars().last(), Some('=' | '-' | '+' | '*' | '/' | '^')) {
			string.pop();
		}
		string = REMOVE_OPERATIONS_BEFORE_CLOSING_BRACKETS.replace_all(&string, "$2").to_string();
		string
	}

	fn update_calculation_result(&mut self) {
		let input = self.get_processed_input();
		
		self.ui_state.calculation_panel.calculation_result = input.is_empty().not().then(|| {
			let run_result = self.backend.dry_run(&input);
			self.ui_state.generate_result_string(run_result, &mut self.backend)
		});
	}

	fn try_apply_calculation(&mut self) {
		let input = self.get_processed_input();
		if input.is_empty() {
			return;
		}

		let RunResult::Ok(action) = self.backend.run(&input) else {
			return;
		};

		match action {
			RunSuccess::AddedSymbol(symbol) => {
				self.ui_state.update_all_symbol_strings(self.backend.formula_store());
				self.ui_state.add_symbol_definition_to_history(symbol, &mut self.backend);
				self.ui_state.calculation_panel.calculation_input.top_user_input.clear();
				self.update_calculation_result();
			},
			RunSuccess::CalculationResult(result) => {
				if self.ui_state.last_history_entry_matches(&input) {
					return;
				}
				self.ui_state.add_calculation_to_history(input, self.ui_state.create_result_string(result));
			},
		}
	}

	pub fn get_autocompletion_result<'a>(
		input_string: &str, cursor_pos: usize, formula_store: &'a FormulaStore,
	) -> Option<AutocompletionResult<'a>> {
		let input_symbol_name = get_fun_name_end_of_string(input_string.char_range(0..cursor_pos), false);
		if input_symbol_name.is_empty() {
			return None;
		}
		let compatible_symbols = formula_store
			.get_symbols_sorted()
			.into_iter()
			.filter(|(name, _)| name.starts_with(&input_symbol_name))
			.collect::<Vec<_>>();

		if compatible_symbols.is_empty() {
			return None;
		}
		let longest_common_start = logic::determine_longest_common_start(&compatible_symbols);
		Some(AutocompletionResult {
			input_term: input_symbol_name,
			possible_symbols: compatible_symbols,
			longest_common_start,
		})
	}

	pub fn show_background_context_menu(&mut self, ctx: &Context, ui: &mut Ui) {
		let mut extended_button = |str: &str| Button::new(str).wrap_mode(TextWrapMode::Extend).ui(ui);

		if extended_button(if self.window_state.is_pinned() {
			"⏷ Unpin the window"
		} else {
			"📌 Pin the window"
		})
		.clicked()
		{
			self.switch_window_pinning(ctx);
		}

		if extended_button("❌ Quit").clicked() {
			exit(0);
		}
	}
}

impl Window {
	fn preprocess_brackets(&mut self, ui: &mut Ui) {
		let Some(mut typed_brackets) = ui.input_mut(|ip| {
			ip.events.retain(|e| match e {
				Event::Text(text) => text.is_ascii(),
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

		let Some(mut state) = TextEdit::load_state(
			ui.ctx(),
			self.ui_state.calculation_panel.calculation_input.top_user_input_id,
		) else {
			return;
		};
		let Some(cursors) = state.cursor.char_range() else {
			return;
		};
		if let Some(cursor_pos) = cursors.single().map(|c| c.index) {
			let move_cursor = typed_brackets.len();
			if cursor_pos == self.ui_state.calculation_panel.calculation_input.top_user_input.len() {
				typed_brackets += ")";
			}
			self.ui_state
				.calculation_panel
				.calculation_input
				.top_user_input
				.insert_str(cursor_pos, &typed_brackets);
			state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(cursor_pos + move_cursor))));
		} else {
			let [min, max] = cursors.sorted_cursors().map(|c| c.index);
			let string_before = &self.ui_state.calculation_panel.calculation_input.top_user_input[..min];
			let string_middle = &self.ui_state.calculation_panel.calculation_input.top_user_input[min..max];
			let string_after = &self.ui_state.calculation_panel.calculation_input.top_user_input[max..];
			self.ui_state.calculation_panel.calculation_input.top_user_input =
				format!("{}({}){}", string_before, string_middle, string_after);
			state.cursor.set_char_range(Some(CCursorRange::two(
				CCursor::new(cursors.primary.index + 1),
				CCursor::new(cursors.secondary.index + 1),
			)));
		}
		state.store(ui.ctx(), self.ui_state.calculation_panel.calculation_input.top_user_input_id);
	}

	fn preprocess_paste(&mut self, ui: &mut Ui) {
		ui.input_mut(|i| {
			for s in i.events.iter_mut().flat_map(|e| quick_match!(e, Event::Paste(s) => s)) {
				*s = s.trim().to_string();
				debug_print!("Pasted text {s}");
				if s.is_empty() {
					continue;
				}
				match convert_from_latex_if_needed(s) {
					Some(Ok(new_s)) => {
						*s = new_s;
					},
					Some(Err(err)) => {
						only_in_debug!(dbg!(err));
						// todo print error as notification
					},
					_ => {},
				}
			}
		});
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

pub fn get_cursor_pos<'a>(response: impl Into<CursorPosSource<'a>>) -> Option<usize> {
	let source = response.into();
	let state = TextEdit::load_state(source.ctx(), source.id())?;
	state.cursor.char_range().and_then(|c| c.single()).map(|c| c.index)
}

pub struct CursorPosSource<'a>(&'a Context, Id);

impl<'a> CursorPosSource<'a> {
	fn ctx(&self) -> &Context {
		self.0
	}

	fn id(&self) -> Id {
		self.1
	}
}

impl<'a> From<&'a Response> for CursorPosSource<'a> {
	fn from(value: &'a Response) -> Self {
		Self(&value.ctx, value.id)
	}
}

impl<'a> From<&'a TextEditOutput> for CursorPosSource<'a> {
	fn from(value: &'a TextEditOutput) -> Self {
		Self::from(&value.response)
	}
}

impl<'a> From<(&'a Context, Id)> for CursorPosSource<'a> {
	fn from(value: (&'a Context, Id)) -> Self {
		Self(value.0, value.1)
	}
}

pub fn get_cursor_range<'a>(response: impl Into<CursorPosSource<'a>>) -> Option<CCursorRange> {
	let source = response.into();
	let state = TextEdit::load_state(source.ctx(), source.id())?;
	state.cursor.char_range()
}

pub struct TaskSender(Sender<UiInteraction>);
pub struct TaskReceiver(Receiver<UiInteraction>);

pub fn new_task_channel() -> (TaskSender, TaskReceiver) {
	let (t, r) = channel();
	(TaskSender(t), TaskReceiver(r))
}

impl TaskSender {
	pub fn send(&self, task: UiInteraction) {
		self.0.send(task).unwrap();
	}
}

impl AsRef<Receiver<UiInteraction>> for TaskReceiver {
	fn as_ref(&self) -> &Receiver<UiInteraction> {
		&self.0
	}
}

impl AsMut<Receiver<UiInteraction>> for TaskReceiver {
	fn as_mut(&mut self) -> &mut Receiver<UiInteraction> {
		&mut self.0
	}
}
