pub mod bottom_panel;
mod calculation_panel;

use crate::controller::{TaskReceiver, TaskSender, new_task_channel};
use crate::ui::bottom_panel::BottomPanel;
use crate::ui::calculation_panel::CalculationPanel;
use egui::Widget;
use std::cmp::PartialEq;
use std::fmt::Display;

pub(super) struct UiState {
    pub(super) calculation_panel: CalculationPanel,
    pub bottom_panel: BottomPanel,
    pub interaction_sender: TaskSender,
    pub interaction: TaskReceiver,
}

impl UiState {
    pub(super) fn empty() -> Self {
        let (t, r) = new_task_channel();
        Self {
            calculation_panel: CalculationPanel::new(),
            bottom_panel: BottomPanel::new(),
            interaction_sender: t,
            interaction: r,
        }
    }
}
