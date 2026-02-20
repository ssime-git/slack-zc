pub struct Keybinds;

impl Default for Keybinds {
    fn default() -> Self {
        Self
    }
}

impl Keybinds {
    pub fn help_text(&self) -> String {
        r#"Keyboard Shortcuts:

Navigation:
  ↑ / ↓         Scroll messages
  Alt + ↑/↓     Previous/next channel
  Click         Select channel or workspace

Workspaces:
  Ctrl + W      Switch workspace
  Ctrl + N      Add new workspace
  Ctrl + Shift + W  Workspace picker

Search & Commands:
  Ctrl + K      Channel search
  Ctrl + F      Search in current channel

Agent:
  /             Start agent command
  @zeroclaw     Mention agent

General:
  ?             Toggle this help
  Shift + E     Show latest error details
  Ctrl + Q      Quit
  Ctrl + T      Cycle theme

Mouse:
  Scroll        Scroll messages
  Click + drag  Resize panels
"#
        .to_string()
    }
}
