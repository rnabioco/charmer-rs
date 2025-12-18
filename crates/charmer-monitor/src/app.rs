//! Main TUI application.

use charmer_state::PipelineState;

pub struct App {
    pub state: PipelineState,
    pub should_quit: bool,
    pub selected_job: Option<usize>,
}

impl App {
    pub fn new(state: PipelineState) -> Self {
        Self {
            state,
            should_quit: false,
            selected_job: None,
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn select_next(&mut self) {
        // TODO: Implement job selection
    }

    pub fn select_previous(&mut self) {
        // TODO: Implement job selection
    }
}
