use crate::Window;
use crate::controller::{get_cursor_pos, get_cursor_range};
use crate::logic::{Backend, UiInteraction};
use crate::ui::UiState;
use eframe::emath::{Align2, Pos2, Rect, Vec2};
use eframe::epaint::text::cursor::CCursor;
use eframe::epaint::text::{LayoutJob, TextFormat, TextWrapping};
use eframe::epaint::{Color32, FontId};
use egui::containers::menu::{MenuButton, MenuConfig};
use egui::text::CCursorRange;
use egui::text_edit::TextEditOutput;
use egui::{
	DragValue, FontSelection, Id, Key, Label, PopupCloseBehavior, Response, RichText, Sides, TextEdit, Ui,
	Widget,
};
use library::{
	DynamicResult, FormattingOptions, FormulaStore, RunError, RunResult, RunSuccess, StringWithInfo,
	create_default_context,
};
use std::ops::Not;

pub struct CalculationPanel {
	pub(crate) calculation_input: CalculationInput,
	pub(crate) calculation_result: Option<Result<OutputString, String>>,
	pub(super) rounding_digits: usize,
}

impl CalculationPanel {
	pub fn new() -> Self {
		Self {
			calculation_input: CalculationInput::new(),
			calculation_result: None,
			rounding_digits: FormattingOptions::default().round_to_decimals,
		}
	}
}

#[derive(Debug)]
pub enum OutputString {
	/// A string representing the definition of a symbol, along with an optional value
	SymbolDefinition(String, Option<StringWithInfo>),
	Result(StringWithInfo),
}

pub(crate) struct CalculationInput {
	pub(crate) top_user_input: String,
	pub(crate) top_user_input_id: Id,
	#[allow(unused)]
	top_input_cache: InputCache,
}

impl CalculationInput {
	pub fn new() -> Self {
		Self {
			top_user_input: "".to_owned(),
			top_user_input_id: "Formula Input".into(),
			top_input_cache: InputCache { last_input: "".to_owned(), last_cursor_range: None },
		}
	}
}

#[derive(PartialEq)]
struct InputCache {
	last_input: String,
	last_cursor_range: Option<CursorRange>,
}

#[derive(PartialEq)]
enum CursorRange {
	#[allow(unused)]
	Single(usize),
	#[allow(unused)]
	Range(usize, usize),
}

pub(super) trait MulReplacement {
	fn replace_mul(&self) -> String;
	fn unreplace_mul(&self) -> String;
}

impl MulReplacement for String {
	fn replace_mul(&self) -> String {
		self.replace("*", "×")
	}

	fn unreplace_mul(&self) -> String {
		self.replace("×", "*")
	}
}

pub fn last_caret_pos_from_output(output: &TextEditOutput) -> Pos2 {
	output.galley.rect.right_top() + output.galley_pos.to_vec2()
}

const TEXTEDIT_FONT_ID: FontId = FontId::proportional(22.0);

#[inline]
fn bracket_highlight_color() -> Color32 {
	Color32::GREEN
}

/// Determine all the parts that are in the same brackets as the cursor
fn find_bracket_area(str: &str, cursor_range: CCursorRange) -> Option<(usize, usize)> {
	let [min, max] = cursor_range.sorted_cursors().map(|c| c.index);
	let outest_idx = if min == max { min } else { determine_outest_cursor_pos(str, min, max) };
	find_bracket_range(str, outest_idx)
}

/// Determine the position between the two cursor indices where the scope is the broadest
fn determine_outest_cursor_pos(str: &str, min: usize, max: usize) -> usize {
	let mut outest_idx = min;
	let mut outest_layer = 0;
	let mut layer = 0;
	for i in min..max {
		// the ' ' character is used to prevent the program from crashing (see #35)
		layer += match str.chars().nth(i).unwrap_or(' ') {
			')' => -1,
			'(' => 1,
			_ => continue,
		};
		if layer < outest_layer {
			outest_layer = layer;
			outest_idx = i + 1;
		}
	}
	outest_idx
}

fn find_bracket_range(str: &str, idx: usize) -> Option<(usize, usize)> {
	let chars: Vec<_> = str.chars().collect();
	if idx > chars.len() {
		return None;
	}
	let mut min = 0;
	let mut indenting = 0;
	for i in (0..idx).rev() {
		indenting += match chars[i] {
			')' => 1,
			'(' => -1,
			_ => continue,
		};
		if indenting < 0 {
			min = i + 1;
			break;
		}
	}
	indenting = 0;
	let mut max = chars.len();
	for (i, &ch) in chars.iter().enumerate().skip(idx) {
		indenting += match ch {
			')' => -1,
			'(' => 1,
			_ => continue,
		};
		if indenting < 0 {
			max = i;
			break;
		}
	}
	if min == 0 && max == chars.len() { None } else { Some((min, max)) }
}

impl UiState {
	pub fn generate_output_string(
		&self, result: RunResult, backend: &mut Backend,
	) -> Result<OutputString, String> {
		match result {
			RunResult::Err(RunError::ParseFailed(s)) => Err(format!("Parse Error: {}", s)),
			RunResult::Err(RunError::CalculationFailed(s)) => Err(format!("Calculation Error: {}", s)),
			RunResult::Err(RunError::FailedToAddSymbol(s)) => Err(format!("Error adding symbol: {}", s)),
			RunResult::Ok(RunSuccess::CalculationResult(r)) => {
				Ok(OutputString::Result(self.format_number_result(r)))
			},
			RunResult::Ok(RunSuccess::AddedSymbol(s)) => {
				let string = s.get_full_string(&mut create_default_context(), false).replace_mul();
				let value = s
					.symbol()
					.formula()
					.is_number()
					.not()
					.then(|| {
						backend
							.evaluate(dbg!(s.symbol().formula().clone()))
							.flatten()
							.ok()
							.map(|d| self.format_number_result(d))
					})
					.flatten();
				Ok(OutputString::SymbolDefinition(format!("Create new symbol: {}", string), value))
			},
		}
	}

	pub fn format_number_result(&self, r: DynamicResult) -> StringWithInfo {
		StringWithInfo::new(r, self.calculation_panel.rounding_digits)
	}

	pub(crate) fn show_top_input_textedit(&mut self, ui: &mut Ui) -> TextEditOutput {
		// todo do bracket highlighting in here
		let _cursor_pos =
			get_cursor_range((ui.ctx(), self.calculation_panel.calculation_input.top_user_input_id));
		self.calculation_panel.calculation_input.top_user_input =
			self.calculation_panel.calculation_input.top_user_input.replace_mul();
		let response = TextEdit::singleline(&mut self.calculation_panel.calculation_input.top_user_input)
			.id(self.calculation_panel.calculation_input.top_user_input_id)
			.hint_text("Enter formula here ...")
			.font(FontSelection::FontId(TEXTEDIT_FONT_ID))
			.lock_focus(true)
			.desired_width(ui.available_width())
			.frame(false)
			.show(ui);
		self.calculation_panel.calculation_input.top_user_input =
			self.calculation_panel.calculation_input.top_user_input.unreplace_mul();

		ui.separator();
		response
	}

	pub fn show_inline_result(&self, ui: &mut Ui, output: &TextEditOutput) -> Option<()> {
		let pos = last_caret_pos_from_output(output);
		println!("Showing inline result at pos {:?}", pos);
		let calculation_result = self.calculation_panel.calculation_result.as_ref()?.as_ref().ok()?;
		println!("Calculation result: {:?}", calculation_result);
		let text = match calculation_result {
			OutputString::SymbolDefinition(_, Some(value)) => &value.main,
			OutputString::Result(StringWithInfo { main, .. }) => main,
			_ => return None,
		};

		ui.painter_at(output.response.rect).text(
			pos,
			Align2::LEFT_TOP,
			" ".to_owned() + text,
			TEXTEDIT_FONT_ID,
			ui.visuals().text_color(),
		);
		Some(())
	}

	pub fn show_brackets_highlighting(&self, ui: &mut Ui, output: &TextEditOutput) -> Option<()> {
		let cursor_range = get_cursor_range(&output.response)?;
		let (min, max) =
			find_bracket_area(&self.calculation_panel.calculation_input.top_user_input, cursor_range)?;
		let left = output.galley.pos_from_cursor(CCursor::new(min));
		let right = output.galley.pos_from_cursor(CCursor::new(max));
		let offset = output.galley_pos.to_vec2();
		let r = Rect::from_min_max(left.max, right.max).translate(offset + Vec2::DOWN * 2.0);
		// ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, ui.visuals().text_color()));
		ui.painter().rect_filled(r.expand2(Vec2::new(0.0, 1.0)), 2.0, bracket_highlight_color());
		Self::draw_bracket(ui, left, offset, true, min != 0);
		Self::draw_bracket(
			ui,
			right,
			offset,
			false,
			max != self.calculation_panel.calculation_input.top_user_input.len(),
		);
		Some(())
	}

	fn draw_bracket(ui: &mut Ui, left: Rect, offset: Vec2, is_left: bool, valid: bool) {
		ui.painter().text(
			left.left_top() + offset,
			if is_left { Align2::RIGHT_TOP } else { Align2::LEFT_TOP },
			if is_left { '(' } else { ')' },
			TEXTEDIT_FONT_ID,
			if valid { bracket_highlight_color() } else { Color32::ORANGE.gamma_multiply(0.5) },
		);
	}

	pub fn show_result_label(&mut self, ui: &mut Ui) {
		// todo only show an error icon and display the error message in a tooltip

		let mut format = TextFormat { font_id: FontId::proportional(20.0), ..Default::default() };
		if self.calculation_panel.calculation_result.as_ref().is_some_and(|r| r.is_err()) {
			format.color = Color32::ORANGE.gamma_multiply(0.7);
		}
		let mut small_format = TextFormat { font_id: FontId::proportional(15.0), ..Default::default() };
		small_format.color = small_format.color.gamma_multiply(0.7);

		let mut job = LayoutJob::default();
		match &self.calculation_panel.calculation_result {
			Some(Ok(result)) => {
				let (main, info) = match result {
					OutputString::SymbolDefinition(main, _) => (main, None),
					OutputString::Result(StringWithInfo { main, info }) => (main, info.as_ref()),
				};
				job.append(main, 0.0, format.clone());
				if let Some(info) = info {
					job.append(info, 0.0, small_format.clone());
				}
			},
			Some(Err(_)) => job.append("=  !", 0.0, format.clone()),
			None => job.append("=", 0.0, format.clone()),
		};

		job.wrap = TextWrapping {
			max_width: ui.available_width(),
			max_rows: usize::MAX,
			break_anywhere: true,
			overflow_character: None,
		};
		let error_string = self.calculation_panel.calculation_result.as_ref().and_then(|r| r.as_ref().err());

		Sides::new().shrink_left().show(
			ui,
			|ui| {
				let result_resp = Label::new(job).selectable(error_string.is_none()).wrap().ui(ui);
				if let Some(err) = error_string {
					result_resp.on_hover_text(err);
				}
			},
			|ui| {
				let mut button = MenuButton::new(RichText::new("⛭").size(15.0));
				button.button = button.button.frame(false).min_size(Vec2::new(25.0, 25.0));
				button
					.config(MenuConfig::default().close_behavior(PopupCloseBehavior::CloseOnClickOutside))
					.ui(ui, |ui| {
						ui.horizontal(|ui| {
							ui.label("Round Digits:");
							let drag_val_resp = DragValue::new(&mut self.calculation_panel.rounding_digits)
								.range(1..=100)
								.ui(ui);
							if drag_val_resp.changed() {
								self.interaction_sender.send(UiInteraction::RoundingAccuracyChanged);
							}
						});
					});
			},
		);
	}

	pub(crate) fn show_autocompletion(
		&mut self, ui: &mut Ui, response: &Response, formula_store: &FormulaStore,
	) -> bool {
		let Some(cursor_pos) = get_cursor_pos(response) else { return false };
		let Some(autocompletion) = Window::get_autocompletion_result(
			&self.calculation_panel.calculation_input.top_user_input,
			cursor_pos,
			formula_store,
		) else {
			return false;
		};

		let already_input = autocompletion.input_term.len();

		response.show_tooltip_ui(|ui| {
			for (name, symbol) in autocompletion.possible_symbols {
				let mut job = LayoutJob::default();
				job.append(
					&autocompletion.input_term,
					0.0,
					TextFormat::simple(FontId::default(), ui.visuals().text_color()),
				);
				job.append(
					&symbol.get_signature_string(name)[already_input..],
					0.0,
					TextFormat::simple(FontId::default(), ui.visuals().weak_text_color()),
				);
				Label::new(job).extend().ui(ui);
			}
		});
		if ui.input(|i| i.key_pressed(Key::Tab)) {
			self.interaction_sender.send(UiInteraction::RequestAutocompletion {
				cursor_pos,
				input_term: autocompletion.input_term,
				complete_to: autocompletion.longest_common_start,
				response: response.clone(),
			});
		}
		true
	}
}
