#![doc = include_str!("../README.md")]

use ::std::{convert::identity, path::PathBuf};

use ::clap::{Parser, ValueEnum};
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

use crate::{config::Config, state::State};

pub mod config;

pub mod state;

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
    /// Update config.
    UpdateConfig(Box<(Config, PathBuf)>),
    /// Load config file.
    LoadConfig(PathBuf),
    /// Save config.
    SaveConfig(Box<(Config, PathBuf)>),
    /// Open executable dialog.
    ExeDialog,
    /// Open config dialog.
    LoadConfigDialog,
    /// Save config dialog.
    SaveConfigDialog,
    /// Run executable.
    Run,
    /// Exit.
    Exit,
    /// Reload content to initial input.
    Reload,
}

impl From<String> for Message {
    fn from(value: String) -> Self {
        Self::SetStatus(value)
    }
}

impl Cli {
    /// Run Application.
    ///
    /// # Errors
    /// On fatal application errors.
    pub fn run(mut self) -> ::color_eyre::Result<()> {
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
                    let task = if let Some(path) = self.config_path.take() {
                        Message::LoadConfig(path)
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
                Task::done(
                    format!("set theme to {theme}", theme = ::iced::Theme::from(theme)).into(),
                )
            }
            Message::SetExe(exe) => {
                self.state.exe = exe;
                Task::done(format!("selected {exe}", exe = self.state.exe).into())
            }
            Message::Run => {
                let config = match self.state.to_config() {
                    Ok(config) => config,
                    Err(err) => return Task::done(err.into()),
                };
                Task::future(config.run_async()).then(|result| match result {
                    Ok(status) => Task::done(format!("process finished with {status}").into()),
                    Err(msg) => {
                        ::log::error!("failed to run process\n{msg}");
                        Task::done(msg.to_string().into())
                    }
                })
            }
            Message::ExeDialog => Task::future(
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
            Message::UpdateConfig(config) => {
                let (Config { exe, arg }, path_buf) = *config;

                if !exe.is_empty() {
                    self.config.exe = exe;
                }

                if !arg.is_empty() {
                    self.config.arg = arg;
                }

                Task::batch(
                    [
                        format!("loaded config {path_buf:?}").into(),
                        Message::Reload,
                    ]
                    .map(Task::done),
                )
            }
            Message::LoadConfig(path_buf) => {
                Task::future(Config::load(path_buf)).then(|result| match result {
                    Ok(config) => Task::done(Message::UpdateConfig(Box::new(config))),
                    Err(err) => {
                        ::log::error!("{err}");
                        Task::done(err.into())
                    }
                })
            }
            Message::SaveConfig(config) => {
                let (config, path_buf) = *config;
                Task::future(config.save(path_buf)).then(|result| match result {
                    Ok(path_buf) => Task::done(format!("saved config to {path_buf:?}").into()),
                    Err(err) => {
                        ::log::error!("{err}");
                        Task::done(err.into())
                    }
                })
            }
            Message::LoadConfigDialog => {
                Task::future(Config::load_dialog()).then(|result| match result {
                    Ok(path_buf) => Task::done(Message::LoadConfig(path_buf)),
                    Err(err) => {
                        ::log::error!("{err}");
                        Task::done(err.into())
                    }
                })
            }
            Message::SaveConfigDialog => {
                let config = match self.state.to_config() {
                    Ok(config) => config,
                    Err(err) => return Task::done(err.into()),
                };

                Task::future(async { (config, Config::save_dialog().await) }).then(
                    |(config, result)| match result {
                        Ok(path_buf) => {
                            Task::done(Message::SaveConfig(Box::new((config, path_buf))))
                        }
                        Err(err) => Task::done(err.into()),
                    },
                )
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
                    .push(button("Open").on_press_with(|| Message::ExeDialog)),
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
                    .push(button("Load").on_press_with(|| Message::LoadConfigDialog))
                    .push(button("Reload").on_press_with(|| Message::Reload))
                    .push(button("Cancel").on_press_with(|| Message::Exit))
                    .push(button("Run").on_press_with(|| Message::Run)),
            )
            .into()
    }
}
