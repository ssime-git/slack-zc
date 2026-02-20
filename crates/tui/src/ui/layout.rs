use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::panel::{Panel, PanelType};

#[derive(Debug, Clone, Copy)]
pub enum DragTarget {
    Sidebar,
    AgentPanel,
}

const MIN_SIDEBAR_WIDTH: u16 = 15;
const MAX_SIDEBAR_WIDTH: u16 = 35;
const MIN_AGENT_WIDTH: u16 = 20;
const MAX_AGENT_WIDTH: u16 = 40;
const TOPBAR_HEIGHT: u16 = 1;
const INPUT_HEIGHT: u16 = 3;

pub struct LayoutState {
    sidebar_width: u16,
    agent_width: u16,
    cached_panels: Vec<Panel>,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            sidebar_width: 20,
            agent_width: 26,
            cached_panels: Vec::new(),
        }
    }
}

impl LayoutState {
    pub fn calculate_layout(&mut self, area: Rect) -> &[Panel] {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(TOPBAR_HEIGHT),
                Constraint::Min(1),
                Constraint::Length(INPUT_HEIGHT),
            ])
            .split(area);

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(self.sidebar_width),
                Constraint::Min(40),
                Constraint::Length(self.agent_width),
            ])
            .split(main_layout[1]);

        self.cached_panels = vec![
            Panel {
                panel_type: PanelType::Topbar,
                rect: main_layout[0],
            },
            Panel {
                panel_type: PanelType::Sidebar,
                rect: content_layout[0],
            },
            Panel {
                panel_type: PanelType::Messages,
                rect: content_layout[1],
            },
            Panel {
                panel_type: PanelType::AgentPanel,
                rect: content_layout[2],
            },
            Panel {
                panel_type: PanelType::InputBar,
                rect: main_layout[2],
            },
        ];

        &self.cached_panels
    }

    pub fn get_panels(&self) -> &[Panel] {
        &self.cached_panels
    }

    pub fn handle_drag(&mut self, target: DragTarget, delta: i16) {
        match target {
            DragTarget::Sidebar => {
                let new_width = (self.sidebar_width as i16 + delta)
                    .clamp(MIN_SIDEBAR_WIDTH as i16, MAX_SIDEBAR_WIDTH as i16)
                    as u16;
                self.sidebar_width = new_width;
            }
            DragTarget::AgentPanel => {
                let new_width = (self.agent_width as i16 - delta)
                    .clamp(MIN_AGENT_WIDTH as i16, MAX_AGENT_WIDTH as i16)
                    as u16;
                self.agent_width = new_width;
            }
        }
    }

    pub fn get_sidebar_rect(&self) -> Option<Rect> {
        self.cached_panels
            .iter()
            .find(|p| matches!(p.panel_type, PanelType::Sidebar))
            .map(|p| p.rect)
    }

    pub fn get_agent_rect(&self) -> Option<Rect> {
        self.cached_panels
            .iter()
            .find(|p| matches!(p.panel_type, PanelType::AgentPanel))
            .map(|p| p.rect)
    }
}
