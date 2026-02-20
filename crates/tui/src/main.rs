mod app;
mod config;
mod input;
mod keybinds;
mod onboarding;
mod ui;

use app::App;
use config::Config;
use directories::ProjectDirs;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers,
};
use ratatui::crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

fn get_config_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "slack-zc", "slack-zc") {
        proj_dirs.config_dir().join("config.toml")
    } else {
        PathBuf::from("config/default.toml")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    terminal::enable_raw_mode()?;
    let mut terminal = ratatui::init();
    ratatui::crossterm::execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    let result = run(&mut terminal);

    let _ = ratatui::crossterm::execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    ratatui::restore();

    result
}

fn run(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = get_config_path();
    let config = Config::load_or_default(&config_path);

    let rt = tokio::runtime::Runtime::new()?;
    let mut app = App::new(config.clone());

    rt.block_on(async {
        if let Err(e) = app.init(&config).await {
            eprintln!("Failed to initialize app: {}", e);
        }
    });

    loop {
        terminal.draw(|frame| app.render(frame))?;

        if event::poll(Duration::from_millis(50))? {
            let event = event::read()?;

            if let Event::Key(key) = &event {
                if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
            }

            if let Ok(should_quit) = app.handle_event(event) {
                if should_quit {
                    break;
                }
            }
        }

        app.process_slack_events();

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
