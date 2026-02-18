#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    AgentCommand,
    AgentMention,
}

pub struct InputState {
    pub buffer: String,
    pub mode: InputMode,
    cursor_position: usize,
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputState {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            mode: InputMode::Normal,
            cursor_position: 0,
        }
    }

    pub fn handle_char(&mut self, c: char) {
        if self.buffer.is_empty() && c == '/' {
            self.mode = InputMode::AgentCommand;
        } else if self.buffer.is_empty() && c == '@' {
            self.mode = InputMode::AgentMention;
        }

        self.buffer.push(c);
        self.cursor_position = self.buffer.len();
        self.update_mode();
    }

    pub fn handle_backspace(&mut self) {
        if !self.buffer.is_empty() {
            self.buffer.pop();
            self.cursor_position = self.buffer.len();
            self.update_mode();
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_position = 0;
        self.mode = InputMode::Normal;
    }

    fn update_mode(&mut self) {
        if self.buffer.is_empty() {
            self.mode = InputMode::Normal;
        } else if self.buffer.starts_with('/') {
            self.mode = InputMode::AgentCommand;
        } else if self.buffer.to_lowercase().starts_with("@zeroclaw")
            || self.buffer.to_lowercase().starts_with("@zc")
        {
            self.mode = InputMode::AgentMention;
        } else {
            self.mode = InputMode::Normal;
        }
    }
}
