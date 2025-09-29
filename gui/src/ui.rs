use crate::Window;
use crate::controller::{TaskReceiver, TaskSender, get_cursor_pos, new_task_channel};
use crate::logic::UiInteraction;
use crate::ui::HistoryEntryContent::{Calculation, SymbolDefinition};
use eframe::epaint::text::{LayoutJob, TextFormat, TextWrapping};
use eframe::epaint::{Color32, FontFamily, FontId, Vec2};
use egui::containers::menu::{MenuButton, MenuConfig};
use egui::scroll_area::ScrollBarVisibility;
use egui::style::ScrollStyle;
use egui::text_edit::TextEditOutput;
use egui::{
    Align2, DragValue, FontSelection, Frame, Id, Key, Label, Margin, PopupCloseBehavior, Pos2, Response,
    RichText, ScrollArea, Separator, Shadow, Sides, TextEdit, Ui, Widget,
};
use library::{
    DynamicResult, FormattedCalculationOutput, FormattingOptions, FormulaStore, NamedSymbol, RunError,
    RunResult, RunSuccess, create_default_context,
};
use std::cmp::PartialEq;
use std::fmt::Display;

pub enum OutputString {
    SymbolDefinition(String),
    Result(StringWithInfo),
}

pub struct StringWithInfo {
    pub main: String,
    pub info: Option<String>,
}

pub(super) struct UiState {
    pub(super) top_user_input: String,
    pub(super) top_user_input_id: Id,
    pub(super) calculation_result: Option<Result<OutputString, String>>,
    pub(super) rounding_digits: usize,
    pub all_symbol_strings: Vec<String>,
    pub history: Vec<HistoryEntry>,
    pub selected_page: Page,
    pub interaction_sender: TaskSender,
    pub interaction: TaskReceiver,
}

pub struct HistoryEntry {
    pub content: HistoryEntryContent,
    pub time: String,
}

pub enum HistoryEntryContent {
    Calculation(String, StringWithInfo),
    SymbolDefinition(String),
    ClearedSymbols,
}

impl HistoryEntryContent {
    fn get_symbol(&self) -> char {
        match self {
            Calculation(..) => '🖩',
            SymbolDefinition(_) => '⛃',
            HistoryEntryContent::ClearedSymbols => '🗑',
        }
    }
}

impl HistoryEntry {
    pub fn new_calculation(input: String, result: StringWithInfo) -> Self {
        Self { content: Calculation(input, result), time: Self::get_current_time() }
    }

    pub fn new_symbol_definition(string: String) -> Self {
        Self { content: SymbolDefinition(string), time: Self::get_current_time() }
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

        Sides::new().show(
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
                        SymbolDefinition(text) => {
                            ui.label(regular_format(text));
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
    pub(super) fn empty() -> Self {
        let (t, r) = new_task_channel();
        Self {
            top_user_input: "".to_owned(),
            top_user_input_id: "Formula Input".into(),
            calculation_result: None,
            rounding_digits: FormattingOptions::default().round_to_decimals,
            all_symbol_strings: vec![],
            history: vec![],
            selected_page: Page::History,
            interaction_sender: t,
            interaction: r,
        }
    }

    pub fn add_symbol_definition_to_history(&mut self, symbol: NamedSymbol) {
        self.history.push(HistoryEntry::new_symbol_definition(
            symbol.symbol().get_full_string(symbol.name(), &mut create_default_context()).replace_mul(),
        ));
    }

    pub fn add_calculation_to_history(&mut self, input: String, result: StringWithInfo) {
        self.history.push(HistoryEntry::new_calculation(input.clone().replace_mul(), result))
    }

    pub fn last_history_entry_matches(&self, input: &str) -> bool {
        self.history.last().is_some_and(|entry| {
            matches!(
                &entry.content,
                Calculation(e_input, _) if e_input == input
            )
        })
    }

    pub fn generate_output_string(&self, result: RunResult) -> Result<OutputString, String> {
        match result {
            RunResult::Err(RunError::ParseFailed(s)) => Err(format!("Parse Error: {}", s)),
            RunResult::Err(RunError::CalculationFailed(s)) => Err(format!("Calculation Error: {}", s)),
            RunResult::Err(RunError::FailedToAddSymbol(s)) => Err(format!("Error adding symbol: {}", s)),
            RunResult::Ok(RunSuccess::CalculationResult(r)) => {
                Ok(OutputString::Result(self.format_number_result(r)))
            },
            RunResult::Ok(RunSuccess::AddedSymbol(s)) => {
                let string =
                    s.symbol().get_full_string(s.name(), &mut create_default_context()).replace_mul();
                Ok(OutputString::SymbolDefinition(format!("Create new symbol: {}", string)))
            },
        }
    }

    pub fn format_number_result(&self, r: DynamicResult) -> StringWithInfo {
        format_number_result(r, self.rounding_digits)
    }

    pub fn update_all_symbol_strings(&mut self, formula_store: &FormulaStore) {
        let elements = formula_store.get_symbols_sorted();
        let ctx = &mut create_default_context();
        self.all_symbol_strings =
            elements.iter().map(|(name, symbol)| symbol.get_full_string(name, ctx).replace_mul()).collect();
    }
}

fn format_number_result(r: DynamicResult, rounding_digits: usize) -> StringWithInfo {
    let formatting_options = FormattingOptions::default().with_rounding(rounding_digits);
    let output = r.to_string_detailed(formatting_options);
    match output {
        FormattedCalculationOutput::Exact { result, has_rounded } => StringWithInfo {
            main: format!("= {}", result),
            info: has_rounded.then_some(" (rounded)".to_owned()),
        },
        FormattedCalculationOutput::ApproximationChecked { result, precision_bits } => StringWithInfo {
            main: format!("≈ {}", result),
            info: Some(format!(" ({} bit precision)", precision_bits)),
        },
        FormattedCalculationOutput::ApproximationReachedLimit { result } => StringWithInfo {
            main: format!("≈ {}", result),
            info: Some(" (reached precision limit)".to_owned()),
        },
    }
}

pub fn caret_pos_from_output(output: &TextEditOutput) -> Option<Pos2> {
    let cursor_range = output.cursor_range?; // None => no caret / not focused
    let ccursor = cursor_range.primary;
    let caret_rect_in_galley = output.galley.pos_from_cursor(ccursor); // relative to galley origin
    Some(output.galley_pos + caret_rect_in_galley.left_top().to_vec2())
}

trait MulReplacement {
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

impl UiState {
    pub(crate) fn show_top_input_textedit(&mut self, ui: &mut Ui) -> TextEditOutput {
        let font_id = FontId::new(22.0, FontFamily::Proportional);
        self.top_user_input = self.top_user_input.replace_mul();
        let response = TextEdit::singleline(&mut self.top_user_input)
            .id(self.top_user_input_id)
            .hint_text("Enter formula here ...")
            .font(FontSelection::FontId(font_id.clone()))
            .lock_focus(true)
            .desired_width(ui.available_width())
            .frame(false)
            .show(ui);
        self.top_user_input = self.top_user_input.unreplace_mul();

        ui.separator();
        response
    }

    pub fn show_inline_result(&self, ui: &mut Ui, output: &TextEditOutput) {
        let pos = last_caret_pos_from_output(&output);
        if let Some(Ok(result)) = &self.calculation_result {
            if let OutputString::Result(StringWithInfo { main, .. }) = result {
                ui.painter().text(
                    pos,
                    Align2::LEFT_TOP,
                    " ".to_owned() + &main,
                    FontId::new(22.0, FontFamily::Proportional),
                    ui.visuals().text_color(),
                );
            }
        }
    }

    pub fn show_result_label(&mut self, ui: &mut Ui) {
        // todo only show an error icon and display the error message in a tooltip

        let mut format = TextFormat { font_id: FontId::proportional(20.0), ..Default::default() };
        if self.calculation_result.as_ref().is_some_and(|r| r.is_err()) {
            format.color = Color32::ORANGE.gamma_multiply(0.7);
        }
        let mut small_format = TextFormat { font_id: FontId::proportional(15.0), ..Default::default() };
        small_format.color = small_format.color.gamma_multiply(0.7);

        let mut job = LayoutJob::default();
        match &self.calculation_result {
            Some(Ok(result)) => {
                let (main, info) = match result {
                    OutputString::SymbolDefinition(main) => (main, None),
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
        let error_string = self.calculation_result.as_ref().and_then(|r| r.as_ref().err());

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
                            let drag_val_resp =
                                DragValue::new(&mut self.rounding_digits).range(1..=100).ui(ui);
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
        let Some(cursor_pos) = get_cursor_pos(&response) else { return false };
        let Some(autocompletion) =
            Window::get_autocompletion_result(&self.top_user_input, cursor_pos, formula_store)
        else {
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

    pub fn show_tab_selector(&mut self, ui: &mut Ui) {
        let x = Sides::default().show(
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
        ui.selectable_label(self.selected_page == page, page.to_string())
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
            for text in &self.all_symbol_strings {
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
            if self.history.is_empty() {
                ui.label(
                    RichText::new("Press [ENTER] to add calculation to history or to store a symbol").weak(),
                );
            }

            for (i, entry) in self.history.iter().rev().enumerate() {
                let mut margin = Margin::symmetric(10, 7);
                margin.right += 2;
                Frame::window(ui.style()).shadow(Shadow::NONE).inner_margin(margin).show(ui, |ui| {
                    entry.show(ui);
                });
            }
        });
    }
}
