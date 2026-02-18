pub struct SlackSetupState {
    pub client_id: String,
    pub client_secret: String,
    pub selected_field: Field,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Field {
    ClientId,
    ClientSecret,
}

impl Default for SlackSetupState {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            selected_field: Field::ClientId,
        }
    }
}
