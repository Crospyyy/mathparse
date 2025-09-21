use crate::ui::Page;
use egui::Response;

pub enum UiStateInfo {
    TopInputChanged,
    TopInputSubmit,
    RoundingAccuracyChanged,
    RequestAutocompletion { cursor_pos: usize, input_term: String, complete_to: String, response: Response },
    SelectPage(Page),
    ClearCustomSymbols,
    ClearHistory,
}
