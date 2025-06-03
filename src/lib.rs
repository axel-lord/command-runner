#![doc = include_str!("../README.md")]

use ::std::{
    convert::identity,
    path::{Path, PathBuf},
    process::ExitStatus,
};

use ::clap::{Args, Parser, ValueEnum, ValueHint};
use ::color_eyre::Report;
use ::iced::{
    Alignment::Center,
    Element, Font,
    Length::Fill,
    Task,
    futures::FutureExt,
    widget::{self, Column, Row, button, text, text_editor, text_input},
};
use ::rfd::AsyncFileDialog;
use ::serde::{Deserialize, Serialize};

/// Format a status message
macro_rules! status {
    ($($arg:tt)*) => {
        $crate::Message::SetStatus(format!($($arg)*))
    };
}

/// Application inted for use to run other applications in a wine envirnoment.
#[derive(Debug, Parser)]
#[command(author, version, long_about = None)]
pub struct Cli {
    /// Theme to use for application.
    #[arg(value_enum, long, short, default_value_t)]
    theme: Theme,

    /// Load config from file.
    #[arg(long = "config", short)]
    config_path: Option<PathBuf>,

    /// Load config and do not open ui.
    #[arg(
        long,
        conflicts_with = "exe",
        conflicts_with = "arg",
        conflicts_with = "theme",
        requires = "config_path"
    )]
    skip: bool,

    /// Initial application config.
    #[command(flatten)]
    config: Config,

    /// Application state.
    #[arg(skip)]
    state: State,
}

/// Reloadable application state.
#[derive(Debug, Default)]
pub struct State {
    /// Executable.
    exe: String,
    /// Arguments.
    args: widget::text_editor::Content,
    /// Status line.
    status: String,
}

impl State {
    /// Convert current state to a config.
    fn to_config(&self) -> Result<Config, Message> {
        let arg = ::shell_words::split(&self.args.text()).map_err(|err| {
            ::log::error!("could not parse arguments\n{err}");
            status!("could not parse arguments")
        })?;
        let exe = self.exe.clone();

        Ok(Config { exe, arg })
    }
}

/// Application config.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Args)]
#[serde(default)]
pub struct Config {
    /// Executable path.
    #[arg(long, short, default_value_t, value_hint = ValueHint::FilePath)]
    #[serde(skip_serializing_if = "String::is_empty")]
    exe: String,
    /// Application arguments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    arg: Vec<String>,
}

impl Config {
    /// Save config.
    async fn save_inner(&self, path: &Path) -> Result<(), String> {
        let content = ::toml::to_string_pretty(self).map_err(|err| {
            ::log::error!("could not serialize config\n{err}");
            "could not serialize config".to_owned()
        })?;

        ::tokio::fs::write(path, content).await.map_err(|err| {
            ::log::error!("could not write config to {path:?}\n{err}");
            format!("could not write {path:?}")
        })?;
        Ok(())
    }

    /// Save config.
    async fn save(self, path: PathBuf) -> Task<Message> {
        match self.save_inner(&path).await {
            Ok(()) => Task::done(Message::SetStatus(format!("saved to '{path:?}'"))),
            Err(msg) => Task::done(Message::SetStatus(msg)),
        }
    }

    /// Load config.
    async fn load_inner(path: &Path) -> Result<Self, String> {
        let content = ::tokio::fs::read_to_string(&path).await.map_err(|err| {
            ::log::error!("could not read {path:?} to string\n{err}'");
            format!("could not parse '{path:?}'")
        })?;

        let config = ::toml::from_str(&content).map_err(|err| {
            ::log::error!("could not parse '{path:?}' as toml\n{err}");
            format!("could not parse '{path:?}'")
        })?;

        Ok(config)
    }

    /// Load config.
    async fn load(path: PathBuf) -> Task<Message> {
        match Self::load_inner(&path).await {
            Ok(config) => Task::batch(
                [
                    Message::UpdateConfig(Box::new(config)),
                    Message::SetStatus(format!("loaded config {path:?}")),
                ]
                .map(Task::done),
            ),
            Err(err_msg) => {
                Task::batch([Message::SetStatus(err_msg), Message::Reload].map(Task::done))
            }
        }
    }

    /// Load config dialog.
    async fn load_dialog() -> Message {
        match AsyncFileDialog::new()
            .set_title("Open Config")
            .add_filter("TOML", &["toml"])
            .pick_file()
            .await
        {
            Some(handle) => Message::SetConfigPath(handle.path().to_path_buf()),
            None => Message::SetStatus("no config selected".into()),
        }
    }

    /// Save config dialog.
    async fn save_dialog(self) -> Task<Message> {
        match AsyncFileDialog::new()
            .set_title("Save Config")
            .add_filter("TOML", &["toml"])
            .save_file()
            .await
        {
            Some(handle) => self.save(handle.path().to_path_buf()).await,
            None => Task::done(Message::SetStatus("no file to write to selected".into())),
        }
    }

    /// Run this config in an async context.
    async fn run_async(self) -> std::io::Result<ExitStatus> {
        let Self { exe, arg } = self;
        ::tokio::process::Command::new(exe).args(arg).status().await
    }

    /// Run config.
    fn run(self) -> std::io::Result<ExitStatus> {
        let Self { exe, arg } = self;
        ::std::process::Command::new(exe).args(arg).status()
    }
}

/// Application theme.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum Theme {
    /// Use light theme.
    Light,
    /// Use dark theme
    #[default]
    Dark,
}

impl From<Theme> for ::iced::Theme {
    fn from(value: Theme) -> Self {
        match value {
            Theme::Light => ::iced::Theme::Light,
            Theme::Dark => ::iced::Theme::Dark,
        }
    }
}

/// Application message.
#[derive(Debug, Clone)]
pub enum Message {
    /// Set the active theme.
    SetTheme(Theme),
    /// Set the executable in use.
    SetExe(String),
    /// Edit arguments.
    EditArgs(widget::text_editor::Action),
    /// Set status line.
    SetStatus(String),
    /// Set config path.
    SetConfigPath(PathBuf),
    /// Update config.
    UpdateConfig(Box<Config>),
    /// Load config file.
    LoadConfig,
    /// Open executable dialog.
    OpenExeDialog,
    /// Open config dialog.
    OpenConfigDialog,
    /// Save config dialog.
    SaveConfigDialog,
    /// Run executable.
    Run,
    /// Exit.
    Exit,
    /// Reload content to initial input.
    Reload,
}

impl Cli {
    /// Run Application.
    ///
    /// # Errors
    /// On fatal application errors.
    pub fn run(self) -> ::color_eyre::Result<()> {
        if self.skip {
            let config =
                ::std::fs::read_to_string(self.config_path.unwrap_or_else(|| unreachable!()))?;
            let config = ::toml::from_str::<Config>(&config)?;
            config.run()?;
            Ok(())
        } else {
            iced::application("Run Command", Self::update, Self::view)
                .theme(|cli| ::iced::Theme::from(cli.theme))
                .window_size((500.0, 200.0))
                .centered()
                .executor::<::tokio::runtime::Runtime>()
                .run_with(|| {
                    let task = if self.config_path.is_some() {
                        Message::LoadConfig
                    } else {
                        Message::Reload
                    };
                    (self, Task::done(task))
                })
                .map_err(Report::from)
        }
    }

    /// Update application state.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetTheme(theme) => {
                self.theme = theme;
                Task::done(Message::SetStatus(format!(
                    "set theme to {theme}",
                    theme = ::iced::Theme::from(theme)
                )))
            }
            Message::SetExe(exe) => {
                self.state.exe = exe;
                Task::done(Message::SetStatus(format!(
                    "selected {exe}",
                    exe = self.state.exe
                )))
            }
            Message::Run => {
                let config = match self.state.to_config() {
                    Ok(config) => config,
                    Err(err) => return Task::done(err),
                };
                Task::future(config.run_async()).then(|result| match result {
                    Ok(status) => Task::done(status!("process finished with {status}")),
                    Err(msg) => {
                        ::log::error!("failed to run process\n{msg}");
                        Task::done(status!("{msg}"))
                    }
                })
            }
            Message::OpenExeDialog => Task::future(
                AsyncFileDialog::new()
                    .set_file_name(&self.state.exe)
                    .set_title("Select Executable")
                    .pick_file()
                    .map(|handle| {
                        let handle = handle
                            .ok_or_else(|| Message::SetStatus("no executable selected".into()))?;
                        let path = handle.path();
                        handle
                            .path()
                            .to_str()
                            .map(String::from)
                            .ok_or_else(|| Message::SetStatus(format!("{path:?} is not unicode")))
                    })
                    .map(|exe| exe.map_or_else(identity, Message::SetExe)),
            ),
            Message::EditArgs(action) => {
                self.state.args.perform(action);
                Task::none()
            }
            Message::SetStatus(status) => {
                self.state.status = status;
                Task::none()
            }
            Message::Exit => ::iced::exit(),
            Message::Reload => {
                let Self {
                    theme: _,
                    config: Config { exe, arg },
                    state,
                    config_path: _,
                    skip: _,
                } = self;
                state.args = widget::text_editor::Content::with_text(&::shell_words::join(arg));
                state.exe = exe.clone();

                Task::none()
            }
            Message::LoadConfig => {
                let Some(path) = self.config_path.clone() else {
                    return Task::none();
                };

                Task::future(Config::load(path)).then(identity)
            }
            Message::UpdateConfig(config) => {
                let Config { exe, arg } = *config;

                if !exe.is_empty() {
                    self.config.exe = exe;
                }

                if !arg.is_empty() {
                    self.config.arg = arg;
                }

                Task::done(Message::Reload)
            }
            Message::OpenConfigDialog => Task::future(Config::load_dialog()),
            Message::SetConfigPath(path_buf) => {
                self.config_path = Some(path_buf);
                Task::done(Message::LoadConfig)
            }
            Message::SaveConfigDialog => {
                let config = match self.state.to_config() {
                    Ok(config) => config,
                    Err(err) => return Task::done(err),
                };
                Task::future(Config::save_dialog(config)).then(identity)
            }
        }
    }

    /// Render application.
    pub fn view(&self) -> Element<Message> {
        Column::new()
            .padding(5)
            .spacing(3)
            .width(Fill)
            .height(Fill)
            .align_x(Center)
            .push(
                Row::new()
                    .align_y(Center)
                    .spacing(3)
                    .push(text_input("Executable...", &self.state.exe).on_input(Message::SetExe))
                    .push(button("Open").on_press_with(|| Message::OpenExeDialog)),
            )
            .push(
                text_editor(&self.state.args)
                    .on_action(Message::EditArgs)
                    .font(Font::MONOSPACE)
                    .height(Fill),
            )
            .push(
                Row::new()
                    .spacing(3)
                    .align_y(Center)
                    .push(text(&self.state.status).width(Fill))
                    .push(button("Save").on_press_with(|| Message::SaveConfigDialog))
                    .push(button("Load").on_press_with(|| Message::OpenConfigDialog))
                    .push(button("Reload").on_press_with(|| Message::Reload))
                    .push(button("Cancel").on_press_with(|| Message::Exit))
                    .push(button("Run").on_press_with(|| Message::Run)),
            )
            .into()
    }
}
