use crate::Window;
use crate::controller::get_cursor_pos;
use crate::logic::UiStateInfo;
use crate::ui::HistoryEntryContent::{Calculation, SymbolDefinition};
use eframe::epaint::text::cursor::CCursor;
use eframe::epaint::text::{LayoutJob, TextFormat, TextWrapping};
use eframe::epaint::{Color32, FontFamily, FontId};
use egui::text::CCursorRange;
use egui::text_edit::TextEditOutput;
use egui::{
    Align, Align2, DragValue, FontSelection, Id, Key, Label, LayerId, Layout, Popup, PopupAnchor, Pos2,
    Response, RichText, ScrollArea, Sides, TextEdit, Tooltip, Ui, Widget,
};
use library::FormattingOptions;
use std::cmp::PartialEq;
use std::fmt::Display;

pub(super) struct UiState {
    pub(super) top_user_input: String,
    pub(super) top_user_input_id: Id,
    pub(super) calculation_result: Option<Result<String, String>>,
    pub(super) output_digits: usize,
    pub all_symbol_strings: Vec<String>,
    pub history: Vec<HistoryEntry>,
    pub selected_page: Page,
}

pub struct HistoryEntry {
    pub content: HistoryEntryContent,
    pub time: String,
}

pub enum HistoryEntryContent {
    Calculation(String, String),
    SymbolDefinition(String),
}

impl HistoryEntry {
    pub fn new_calculation(input: String, result: String, time: String) -> Self {
        Self { content: Calculation(input, result), time }
    }

    pub fn new_symbol_definition(string: String, time: String) -> Self {
        Self { content: SymbolDefinition(string), time }
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
        Self {
            top_user_input: "".to_string(),
            top_user_input_id: "Formula Input".into(),
            calculation_result: None,
            output_digits: FormattingOptions::default().round_to_decimals,
            all_symbol_strings: vec![],
            history: vec![],
            selected_page: Page::History,
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

impl Window {
    pub(crate) fn show_top_input_textedit(&mut self, ui: &mut Ui) -> TextEditOutput {
        let font_id = FontId::new(20.0, FontFamily::Proportional);
        self.ui_state.top_user_input = self.ui_state.top_user_input.replace("*", "×");
        let response = TextEdit::singleline(&mut self.ui_state.top_user_input)
            .id(self.ui_state.top_user_input_id)
            .font(FontSelection::FontId(font_id.clone()))
            .lock_focus(true)
            .desired_width(ui.available_width())
            .show(ui);
        self.ui_state.top_user_input = self.ui_state.top_user_input.replace("×", "*");

        response
    }

    pub fn show_textedit_result(&self, ui: &mut Ui, output: &TextEditOutput) {
        let pos = last_caret_pos_from_output(&output);
        if let Some(Ok(result)) = &self.ui_state.calculation_result {
            let result = result.split_once('(').map(|b| b.0.trim()).unwrap_or(result);
            ui.painter().text(
                pos,
                Align2::LEFT_TOP,
                " ".to_string() + &result,
                FontId::new(20.0, FontFamily::Proportional),
                ui.visuals().text_color(),
            );
        }
    }

    pub fn show_result_label(&mut self, ui: &mut Ui, input: &mut Vec<UiStateInfo>) {
        // todo only show an error icon and display the error message in a tooltip

        let mut format = TextFormat { font_id: FontId::proportional(20.0), ..Default::default() };
        if self.ui_state.calculation_result.as_ref().is_some_and(|r| r.is_err()) {
            format.color = Color32::ORANGE.gamma_multiply(0.7);
        }
        let mut small_format = TextFormat { font_id: FontId::proportional(15.0), ..Default::default() };
        small_format.color = small_format.color.gamma_multiply(0.7);

        let mut job = LayoutJob::default();
        match &self.ui_state.calculation_result {
            Some(Ok(result)) => {
                if let Some((first, second)) = result.split_once('(') {
                    job.append(first, 0.0, format);
                    job.append("(", 0.0, small_format.clone());
                    job.append(second, 0.0, small_format);
                } else {
                    job.append(result, 0.0, format);
                }
            },
            Some(Err(_)) => job.append("=  !", 0.0, format),
            None => {},
        };

        job.wrap = TextWrapping {
            max_width: ui.available_width(),
            max_rows: usize::MAX,
            break_anywhere: true,
            overflow_character: None,
        };
        let error_string = self.ui_state.calculation_result.as_ref().and_then(|r| r.as_ref().err());

        Sides::new().show(
            ui,
            |ui| {
                let result_resp = Label::new(job).selectable(error_string.is_none()).ui(ui);
                if let Some(err) = error_string {
                    result_resp.on_hover_text(err);
                }
            },
            |ui| {
                ui.menu_button("⛭", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Round Digits:");
                        let drag_val_resp =
                            DragValue::new(&mut self.ui_state.output_digits).range(1..=100).ui(ui);
                        if drag_val_resp.changed() {
                            input.push(UiStateInfo::RoundingAccuracyChanged);
                        }
                    });
                });
            },
        );
    }

    pub(crate) fn show_autocompletion(
        &mut self, ui: &mut Ui, response: &Response, input: &mut Vec<UiStateInfo>,
    ) -> bool {
        let Some(cursor_pos) = get_cursor_pos(&response) else { return false };
        let Some(autocompletion) = self.get_autocompletion_result(&self.ui_state.top_user_input, cursor_pos)
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
            input.push(UiStateInfo::RequestAutocompletion {
                cursor_pos,
                input_term: autocompletion.input_term,
                complete_to: autocompletion.longest_common_start,
                response: response.clone(),
            });
        }
        true
    }

    pub fn show_tab_selector(&self, ui: &mut Ui, input: &mut Vec<UiStateInfo>) {
        ui.horizontal(|ui| {
            self.add_selectable_label(ui, input, Page::History, "Show calculation history");
            self.add_selectable_label(ui, input, Page::DefinedSymbols, "Show defined symbols");
        });
    }

    fn add_selectable_label(&self, ui: &mut Ui, input: &mut Vec<UiStateInfo>, page: Page, description: &str) {
        ui.selectable_label(self.ui_state.selected_page == page, page.to_string())
            .on_hover_text(description)
            .clicked()
            .then(|| {
                input.push(UiStateInfo::SelectPage(page));
            });
    }

    pub(crate) fn show_all_defined_symbols(&mut self, ui: &mut Ui) {
        // todo align all symbols to the `=` sign
        let area = ScrollArea::vertical().id_salt("defined symbols").auto_shrink(false);
        area.show(ui, |ui| {
            for text in &self.ui_state.all_symbol_strings {
                ui.label(RichText::new(text).size(17.0));
            }
        });
    }

    pub fn show_history(&mut self, ui: &mut Ui) {
        let area = ScrollArea::vertical().id_salt("history").auto_shrink(false);
        area.show(ui, |ui| {
            for text in self.ui_state.history.iter().rev() {
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
