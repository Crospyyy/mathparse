use crate::logic::UiInteraction;
use crate::ui::UiState;
use crate::ui::bottom_panel::HistoryEntryContent::{Calculation, SymbolDefinition};
use crate::ui::calculation_panel::{MulReplacement, StringWithInfo};
use eframe::epaint::text::TextWrapMode;
use eframe::epaint::{Margin, Shadow};
use egui::style::ScrollStyle;
use egui::{Frame, RichText, ScrollArea, Sides, Ui};
use library::{DynamicResult, FormattingOptions, FormulaStore, NamedSymbol, create_default_context};
use std::fmt::Display;

pub struct BottomPanel {
	pub selected_page: Page,
	pub history: Vec<HistoryEntry>,
	pub all_symbol_strings: Vec<String>,
}

impl BottomPanel {
	pub fn new() -> Self {
		Self { all_symbol_strings: vec![], history: vec![], selected_page: Page::History }
	}
}

pub struct HistoryEntry {
	pub content: HistoryEntryContent,
	pub time: String,
}

pub enum HistoryEntryContent {
	Calculation(String, StringWithInfo),
	/// The defined symbol string, and optionally its value as string
	SymbolDefinition(String, Option<String>),
	ClearedSymbols,
}

impl HistoryEntryContent {
	fn get_symbol(&self) -> char {
		match self {
			Calculation(..) => '🖩',
			SymbolDefinition(..) => '⛃',
			HistoryEntryContent::ClearedSymbols => '🗑',
		}
	}
}

impl HistoryEntry {
	pub fn new_calculation(input: String, result: StringWithInfo) -> Self {
		Self { content: Calculation(input, result), time: Self::get_current_time() }
	}

	pub fn new_symbol_definition(string: String, value: Option<String>) -> Self {
		Self { content: SymbolDefinition(string, value), time: Self::get_current_time() }
	}

	pub fn cleared_symbols() -> Self {
		Self { content: HistoryEntryContent::ClearedSymbols, time: Self::get_current_time() }
	}

	fn get_current_time() -> String {
		chrono::Local::now().format("%H:%M").to_string()
	}

	fn show(&self, ui: &mut Ui) {
		fn regular_format(text: impl Into<String>) -> RichText {
			RichText::new(text).size(17.0)
		}
		fn smaller_format(text: impl Into<String>) -> RichText {
			RichText::new(text).size(15.0)
		}
		fn smallest_format(text: impl Into<String>) -> RichText {
			RichText::new(text).size(12.0)
		}

		Sides::new().wrap_mode(TextWrapMode::Wrap).shrink_left().show(
			ui,
			|ui| {
				ui.horizontal(|ui| {
					let symbol = self.content.get_symbol();
					ui.label(regular_format(symbol));
					ui.add_space(5.0);
					match &self.content {
						Calculation(input, result) => {
							ui.vertical(|ui| {
								ui.label(regular_format(input).weak());
								ui.horizontal(|ui| {
									ui.label(regular_format(&result.main));
									if let Some(info) = &result.info {
										ui.label(smaller_format(info).weak());
									}
								});
							});
						},
						SymbolDefinition(text, value) => {
							if let Some(value) = value {
								ui.vertical(|ui| {
									ui.label(regular_format(text));
									ui.label(regular_format(format!("= {}", value)));
								});
							} else {
								ui.label(regular_format(text));
							}
						},
						HistoryEntryContent::ClearedSymbols => {
							ui.label(regular_format("Cleared Symbols"));
						},
					}
				})
			},
			|ui| {
				ui.label(smallest_format(&self.time).weak());
			},
		);
	}
}

#[derive(PartialEq)]
pub enum Page {
	History,
	DefinedSymbols,
}

impl Display for Page {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Page::History => write!(f, "History"),
			Page::DefinedSymbols => write!(f, "Defined Symbols"),
		}
	}
}

impl UiState {
	pub fn add_symbol_definition_to_history(&mut self, symbol: NamedSymbol, value: Option<DynamicResult>) {
		let mut value =
			value.map(|r| r.to_string_detailed(FormattingOptions::default()).get_string().to_owned());
		let formula_string = symbol.symbol().formula().get_string(&mut create_default_context());
		if value.as_ref().is_some_and(|value_string| value_string == &formula_string) {
			value = None;
		}
		let symbol_string = symbol
			.symbol()
			.get_full_string(symbol.name(), &mut create_default_context(), false)
			.replace_mul();

		self.bottom_panel.history.push(HistoryEntry::new_symbol_definition(symbol_string, value));
	}

	pub fn add_calculation_to_history(&mut self, input: String, result: StringWithInfo) {
		self.bottom_panel.history.push(HistoryEntry::new_calculation(input.clone().replace_mul(), result))
	}

	pub fn last_history_entry_matches(&self, input: &str) -> bool {
		self.bottom_panel.history.last().is_some_and(|entry| {
			matches!(
				&entry.content,
				Calculation(e_input, _) if e_input == input
			)
		})
	}

	pub fn update_all_symbol_strings(&mut self, formula_store: &FormulaStore) {
		let elements = formula_store.get_symbols_sorted();
		let ctx = &mut create_default_context();
		self.bottom_panel.all_symbol_strings = elements
			.iter()
			.map(|(name, symbol)| symbol.get_full_string(name, ctx, false).replace_mul())
			.collect();
	}

	pub fn show_tab_selector(&mut self, ui: &mut Ui) {
		Sides::default().show(
			ui,
			|ui| {
				self.add_selectable_label(ui, Page::History, "Show calculation history");
				self.add_selectable_label(ui, Page::DefinedSymbols, "Show defined symbols");
			},
			|ui| {
				if ui.button("Clear Symbols").on_hover_text("Remove all self defined symbols").clicked() {
					self.interaction_sender.send(UiInteraction::ClearCustomSymbols);
				};
				if ui.button("Clear History").clicked() {
					self.interaction_sender.send(UiInteraction::ClearHistory);
				};
			},
		);
	}

	fn add_selectable_label(&self, ui: &mut Ui, page: Page, description: &str) {
		ui.selectable_label(self.bottom_panel.selected_page == page, page.to_string())
			.on_hover_text(description)
			.clicked()
			.then(|| {
				self.interaction_sender.send(UiInteraction::SelectPage(page));
			});
	}

	pub(crate) fn show_all_defined_symbols(&mut self, ui: &mut Ui) {
		// todo align all symbols to the `=` sign
		let mut x = ui.style().as_ref().clone();
		x.spacing.scroll = ScrollStyle::solid();
		ui.set_style(x);
		let area = ScrollArea::vertical().id_salt("defined symbols").auto_shrink([false, true]);
		area.show(ui, |ui| {
			for text in &self.bottom_panel.all_symbol_strings {
				ui.label(RichText::new(text).size(17.0));
			}
		});
	}

	pub fn show_history(&mut self, ui: &mut Ui) {
		let mut x = ui.style().as_ref().clone();
		x.spacing.scroll = ScrollStyle::solid();
		ui.set_style(x);

		let area = ScrollArea::vertical().id_salt("history").auto_shrink([false, true]);
		area.show(ui, |ui| {
			if self.bottom_panel.history.is_empty() {
				ui.label(
					RichText::new("Press [ENTER] to add calculation to history or to store a symbol").weak(),
				);
			}

			for entry in self.bottom_panel.history.iter().rev() {
				let mut margin = Margin::symmetric(10, 7);
				margin.right += 2;
				Frame::window(ui.style()).shadow(Shadow::NONE).inner_margin(margin).show(ui, |ui| {
					entry.show(ui);
				});
			}
		});
	}
}
