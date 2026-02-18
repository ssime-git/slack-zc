#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OnboardingScreen {
    Welcome,
    SlackCredentials,
    OAuthFlow,
    ZeroClawCheck,
    ZeroClawPairing,
    Complete,
}

pub struct OnboardingState {
    pub current_screen: OnboardingScreen,
    pub client_id: String,
    pub client_secret: String,
    pub selected_field: usize,
    pub oauth_url: Option<String>,
    pub oauth_code: String,
    pub pairing_code: Option<String>,
    pub error_message: Option<String>,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self::new()
    }
}

impl OnboardingState {
    pub fn new() -> Self {
        Self {
            current_screen: OnboardingScreen::Welcome,
            client_id: String::new(),
            client_secret: String::new(),
            selected_field: 0,
            oauth_url: None,
            oauth_code: String::new(),
            pairing_code: None,
            error_message: None,
        }
    }

    pub fn generate_oauth_url(&mut self, redirect_port: u16) -> String {
        let url = format!(
            "https://slack.com/oauth/v2/authorize?client_id={}&scope=channels:read,channels:history,channels:write,groups:read,groups:history,groups:write,im:read,im:history,im:write,mpim:read,mpim:history,mpim:write,chat:write,users:read,reactions:read,connections:write&redirect_uri=http://localhost:{}",
            self.client_id, redirect_port
        );
        self.oauth_url = Some(url.clone());
        url
    }

    pub fn toggle_field(&mut self) {
        self.selected_field = (self.selected_field + 1) % 2;
    }

    pub fn current_field_value(&mut self) -> &mut String {
        if self.selected_field == 0 {
            &mut self.client_id
        } else {
            &mut self.client_secret
        }
    }

    pub fn next_screen(&mut self) {
        self.current_screen = match self.current_screen {
            OnboardingScreen::Welcome => OnboardingScreen::SlackCredentials,
            OnboardingScreen::SlackCredentials => OnboardingScreen::OAuthFlow,
            OnboardingScreen::OAuthFlow => OnboardingScreen::ZeroClawCheck,
            OnboardingScreen::ZeroClawCheck => OnboardingScreen::ZeroClawPairing,
            OnboardingScreen::ZeroClawPairing => OnboardingScreen::Complete,
            OnboardingScreen::Complete => OnboardingScreen::Complete,
        };
    }

    pub fn previous_screen(&mut self) {
        self.current_screen = match self.current_screen {
            OnboardingScreen::Welcome => OnboardingScreen::Welcome,
            OnboardingScreen::SlackCredentials => OnboardingScreen::Welcome,
            OnboardingScreen::OAuthFlow => OnboardingScreen::SlackCredentials,
            OnboardingScreen::ZeroClawCheck => OnboardingScreen::OAuthFlow,
            OnboardingScreen::ZeroClawPairing => OnboardingScreen::ZeroClawCheck,
            OnboardingScreen::Complete => OnboardingScreen::ZeroClawPairing,
        };
    }
}
