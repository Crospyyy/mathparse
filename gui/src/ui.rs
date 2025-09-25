use crate::Window;
use crate::controller::{TaskReceiver, TaskSender, get_cursor_pos, new_task_channel};
use crate::logic::UiInteraction;
use crate::ui::HistoryEntryContent::{Calculation, SymbolDefinition};
use eframe::epaint::text::{LayoutJob, TextFormat, TextWrapping};
use eframe::epaint::{Color32, FontFamily, FontId, Vec2};
use egui::containers::menu::{MenuButton, MenuConfig};
use egui::text_edit::TextEditOutput;
use egui::{
    Align2, DragValue, FontSelection, Id, Key, Label, PopupCloseBehavior, Pos2, Response, RichText,
    ScrollArea, Sides, TextEdit, Ui, Widget,
};
use library::{FormattingOptions, FormulaStore};
use std::cmp::PartialEq;
use std::fmt::Display;
use std::sync::mpsc::{Sender, channel};

pub(super) struct UiState {
    pub(super) top_user_input: String,
    pub(super) top_user_input_id: Id,
    pub(super) calculation_result: Option<Result<String, String>>,
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
    Calculation(String, String),
    SymbolDefinition(String),
    ClearedSymbols,
}

impl HistoryEntry {
    pub fn new_calculation(input: String, result: String) -> Self {
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
    pub(super) fn new() -> Self {
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
}

pub fn caret_pos_from_output(output: &TextEditOutput) -> Option<Pos2> {
    let cursor_range = output.cursor_range?; // None => no caret / not focused
    let ccursor = cursor_range.primary;
    let caret_rect_in_galley = output.galley.pos_from_cursor(ccursor); // relative to galley origin
    Some(output.galley_pos + caret_rect_in_galley.left_top().to_vec2())
}

pub fn last_caret_pos_from_output(output: &TextEditOutput) -> Pos2 {
    output.galley.rect.right_top() + output.galley_pos.to_vec2()
}

impl UiState {
    pub(crate) fn show_top_input_textedit(&mut self, ui: &mut Ui) -> TextEditOutput {
        let font_id = FontId::new(22.0, FontFamily::Proportional);
        self.top_user_input = self.top_user_input.replace("*", "×");
        let response = TextEdit::singleline(&mut self.top_user_input)
            .id(self.top_user_input_id)
            .hint_text("Enter formula here ...")
            .font(FontSelection::FontId(font_id.clone()))
            .lock_focus(true)
            .desired_width(ui.available_width())
            .frame(false)
            .show(ui);
        self.top_user_input = self.top_user_input.replace("×", "*");

        ui.separator();
        response
    }

    pub fn show_inline_result(&self, ui: &mut Ui, output: &TextEditOutput) {
        let pos = last_caret_pos_from_output(&output);
        if let Some(Ok(result)) = &self.calculation_result {
            let result = result.split_once('(').map(|b| b.0.trim()).unwrap_or(result);
            ui.painter().text(
                pos,
                Align2::LEFT_TOP,
                " ".to_owned() + &result,
                FontId::new(22.0, FontFamily::Proportional),
                ui.visuals().text_color(),
            );
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
                if let Some((first, second)) = result.split_once('(') {
                    job.append(first, 0.0, format.clone());
                    job.append("(", 0.0, small_format.clone());
                    job.append(second, 0.0, small_format);
                } else {
                    job.append(result, 0.0, format.clone());
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

        Sides::new().show(
            ui,
            |ui| {
                let result_resp = Label::new(job).selectable(error_string.is_none()).ui(ui);
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
            |ui| match self.selected_page {
                Page::History => (ui.button("Clear Symbols").clicked(), ui.button("Clear History").clicked()),
                Page::DefinedSymbols => (false, false),
            },
        );
        // todo change input to channels or something similar
        if x.1.0 {
            self.interaction_sender.send(UiInteraction::ClearCustomSymbols)
        }
        if x.1.1 {
            self.interaction_sender.send(UiInteraction::ClearHistory)
        }
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
        let area = ScrollArea::vertical().id_salt("defined symbols").auto_shrink(false);
        area.show(ui, |ui| {
            for text in &self.all_symbol_strings {
                ui.label(RichText::new(text).size(17.0));
            }
        });
    }

    pub fn show_history(&mut self, ui: &mut Ui) {
        let area = ScrollArea::vertical().id_salt("history").auto_shrink(false);
        area.show(ui, |ui| {
            for text in self.history.iter().rev() {
                ui.group(|ui| {
                    Sides::new().show(
                        ui,
                        |ui| match &text.content {
                            Calculation(input, result) => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("🖩").size(17.0));
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(input).weak().size(17.0));
                                        ui.label(RichText::new(result).size(17.0));
                                    });
                                });
                            },
                            SymbolDefinition(text) => {
                                ui.label(RichText::new(text).size(17.0));
                            },
                            HistoryEntryContent::ClearedSymbols => {
                                ui.label(RichText::new("🗑 Cleared Symbols").size(17.0));
                            },
                        },
                        |ui| {
                            ui.label(RichText::new(&text.time).weak().size(12.0));
                        },
                    );
                });
            }
        });
    }
}
