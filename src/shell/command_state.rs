use crate::shell::osc::OscEvent;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandState {
    pub prompt_line: Option<u16>,
    pub prompt_end_x: Option<u16>,
    pub cwd: String,
    pub command_text: String,
    pub in_prompt: bool,
    pub has_output: bool,
}

impl CommandState {
    pub fn handle_osc(&mut self, event: OscEvent, current_cursor_y: u16, current_cursor_x: u16) {
        match event {
            OscEvent::PromptStarted => {
                self.in_prompt = true;
                self.has_output = false;
                self.prompt_line = Some(current_cursor_y);
                self.prompt_end_x = None;
                self.command_text.clear();
            }
            OscEvent::PromptEnded => {
                self.in_prompt = false;
                self.prompt_line = Some(current_cursor_y);
                self.prompt_end_x = Some(current_cursor_x);
            }
            OscEvent::Cwd(cwd) => {
                self.cwd = cwd;
            }
        }
    }
}
