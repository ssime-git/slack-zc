pub struct Keybinds;

impl Default for Keybinds {
    fn default() -> Self {
        Self
    }
}

impl Keybinds {
    pub fn help_text(&self) -> String {
        r#"Keyboard Shortcuts:

Focus (Tab to cycle):
  Tab           Cycle: Sidebar > Messages > Input
  i             Enter input mode (from Sidebar/Messages)
  Esc           Return to Sidebar focus

Sidebar focus:
  j / Down      Move channel cursor down
  k / Up        Move channel cursor up
  Enter         Open highlighted channel

Messages focus:
  j / Down      Scroll down
  k / Up        Scroll up

Input focus:
  (all keys go to input, no shortcuts)
  Enter         Send message, return to Sidebar
  Esc           Clear input, return to Sidebar

Global (any focus):
  Alt+Up/Down   Switch channel
  Ctrl+W        Workspace picker
  Ctrl+K        Channel search
  Ctrl+C        Copy selected message
  Ctrl+Q        Quit
  ?             Toggle this help

Shortcuts (Sidebar/Messages only):
  t  thread   e  edit   d  delete   D  history
  r  react    g  jump   f  filter   E  error

Agent (in Input focus):
  /             Start agent command
  @zeroclaw     Mention agent

Mouse:
  Click         Select channel / workspace
  Scroll        Scroll messages
  Drag          Resize panels
"#
        .to_string()
    }
}
