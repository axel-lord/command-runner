//! [Config] impl.

use ::std::{path::PathBuf, process::ExitStatus};

use ::clap::{Args, ValueHint};
use ::rfd::AsyncFileDialog;
use ::serde::{Deserialize, Serialize};

use crate::Message;

///  Error raised on save failures.
#[derive(Debug, ::thiserror::Error)]
pub enum SaveError {
    /// Serialization failed.
    #[error("coul not serialize config\n{source}")]
    Serialize {
        /// Path that was to be serialized to.
        path: PathBuf,
        /// Serialization error.
        #[source]
        source: ::toml::ser::Error,
    },

    /// Writing of config failed.
    #[error("could not write config to {path:?}\n{source}")]
    Write {
        /// Path that could not be written.
        path: PathBuf,
        /// Error source.
        #[source]
        source: ::std::io::Error,
    },

    /// No file was selected.
    #[error("no file selected using dialog")]
    NoneSelected,
}

impl From<SaveError> for Message {
    fn from(value: SaveError) -> Self {
        Message::SetStatus(match value {
            SaveError::Serialize { source: _, path: _ } => "could not serialize config".into(),
            SaveError::Write { path, source: _ } => format!("could not write {path:?}"),
            SaveError::NoneSelected => "no path entered".into(),
        })
    }
}

/// Error raised on load failues.
#[derive(Debug, ::thiserror::Error)]
pub enum LoadError {
    /// Deserialization failed.
    #[error("could not parse '{path:?}' as toml\n{source}")]
    Deserialize {
        /// Path that was to be deserialized.
        path: PathBuf,
        /// Deserialization error.
        #[source]
        source: ::toml::de::Error,
    },

    /// Reading of file failed.
    #[error("could not read {path:?} to string\n{source}'")]
    Read {
        /// Path that was read.
        path: PathBuf,
        /// IO error.
        #[source]
        source: ::std::io::Error,
    },

    /// No file was selected in dialog.
    #[error("no file selected using dialog")]
    NoneSelected,
}

impl From<LoadError> for Message {
    fn from(value: LoadError) -> Self {
        Message::SetStatus(match value {
            LoadError::Deserialize { path, source: _ } => format!("could not deserialze {path:?}"),
            LoadError::Read { path, source: _ } => format!("could not read {path:?}"),
            LoadError::NoneSelected => "no file selected".into(),
        })
    }
}

/// Application config.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Args)]
#[serde(default)]
pub struct Config {
    /// Executable path.
    #[arg(long, short, default_value_t, value_hint = ValueHint::FilePath)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub exe: String,
    /// Application arguments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arg: Vec<String>,
}

impl Config {
    /// Save config.
    ///
    /// # Errors
    /// If config cannot be serialized [SaveError::Serialize] is returned.
    /// If serialized config cannot be written [SaveError::Write] is returned.
    pub async fn save(self, path: PathBuf) -> Result<PathBuf, SaveError> {
        match ::toml::to_string_pretty(&self) {
            Ok(content) => match ::tokio::fs::write(&path, &content).await {
                Ok(_) => Ok(path),
                Err(source) => Err(SaveError::Write { path, source }),
            },
            Err(source) => Err(SaveError::Serialize { source, path }),
        }
    }

    /// Load config.
    ///
    /// # Errors
    /// If config serialized config cannot be read [LoadError::Read] is returned.
    /// If config cannot be deserialized [LoadError::Deserialize] is returned.
    pub async fn load(path: PathBuf) -> Result<(Config, PathBuf), LoadError> {
        match ::tokio::fs::read_to_string(&path).await {
            Ok(content) => match ::toml::from_str(&content) {
                Ok(config) => Ok((config, path)),
                Err(source) => Err(LoadError::Deserialize { path, source }),
            },
            Err(source) => Err(LoadError::Read { path, source }),
        }
    }

    /// Load config dialog.
    ///
    /// # Errors
    /// If nothing was selected [LoadError::NoneSelected] is returned.
    pub async fn load_dialog() -> Result<PathBuf, LoadError> {
        match AsyncFileDialog::new()
            .set_title("Open Config")
            .add_filter("TOML", &["toml"])
            .pick_file()
            .await
        {
            Some(handle) => Ok(handle.path().to_path_buf()),
            None => Err(LoadError::NoneSelected),
        }
    }

    /// Save config dialog.
    ///
    /// # Errors
    /// If nothing was selected [SaveError::NoneSelected] is returned.
    pub async fn save_dialog() -> Result<PathBuf, SaveError> {
        match AsyncFileDialog::new()
            .set_title("Save Config")
            .add_filter("TOML", &["toml"])
            .save_file()
            .await
        {
            Some(handle) => Ok(handle.path().to_path_buf()),
            None => Err(SaveError::NoneSelected),
        }
    }

    /// Run this config in an async context.
    ///
    /// # Errors
    /// If the executable cannot be ran.
    pub async fn run_async(self) -> std::io::Result<ExitStatus> {
        let Self { exe, arg } = self;
        ::tokio::process::Command::new(exe).args(arg).status().await
    }

    /// Run config.
    ///
    /// # Errors
    /// If the executable cannot be ran.
    pub fn run(self) -> std::io::Result<ExitStatus> {
        let Self { exe, arg } = self;
        ::std::process::Command::new(exe).args(arg).status()
    }
}
