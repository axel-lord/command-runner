//! [State] impl.
use ::iced::widget::text_editor;

use crate::{Message, config::Config};

/// Reloadable application state.
#[derive(Debug, Default)]
pub struct State {
    /// Executable.
    pub exe: String,
    /// Arguments.
    pub args: text_editor::Content,
    /// Status line.
    pub status: String,
}

impl State {
    /// Convert current state to a config.
    ///
    /// # Errors
    /// If current state cannot be converted to a config.
    pub fn to_config(&self) -> Result<Config, ToConfigError> {
        let arg = ::shell_words::split(&self.args.text())?;
        let exe = self.exe.clone();

        Ok(Config { exe, arg })
    }
}

/// Error raised when current state cannot be parsed to a config.
#[derive(Debug, ::thiserror::Error)]
#[error("could not parse arguments\n{source}")]
pub struct ToConfigError {
    /// Argument parse error.
    #[from]
    source: ::shell_words::ParseError,
}

impl From<ToConfigError> for Message {
    fn from(_value: ToConfigError) -> Self {
        Message::SetStatus("could not parse arguments".into())
    }
}
