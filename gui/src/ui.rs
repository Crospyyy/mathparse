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
        let Some(autocompletion) = self.get_autocompletion_result(&self.ui_state.top_user_input, cursor_pos)
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
