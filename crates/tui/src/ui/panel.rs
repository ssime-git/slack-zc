use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelType {
    Topbar,
    Sidebar,
    Messages,
    AgentPanel,
    InputBar,
}

#[derive(Debug, Clone)]
pub struct Panel {
    pub panel_type: PanelType,
    pub rect: Rect,
}
