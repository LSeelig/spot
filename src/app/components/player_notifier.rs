use std::ops::Deref;
use std::rc::Rc;

use futures::channel::mpsc::UnboundedSender;
use librespot::core::spotify_id::{SpotifyId, SpotifyItemType};

use crate::app::components::EventListener;
use crate::app::state::{
    Device, LoginAction, LoginEvent, LoginStartedEvent, PlaybackEvent, SettingsEvent,
};
use crate::app::{ActionDispatcher, AppAction, AppEvent, AppModel, SongsSource};
use crate::connect::ConnectCommand;
use crate::player::Command;

enum CurrentlyPlaying {
    WithSource {
        source: SongsSource,
        offset: usize,
        song: String,
    },
    Songs {
        songs: Vec<String>,
        offset: usize,
    },
}

impl CurrentlyPlaying {
    fn song_id(&self) -> &String {
        match self {
            Self::WithSource { song, .. } => song,
            Self::Songs { songs, offset } => &songs[*offset],
        }
    }
}

pub struct PlayerNotifier {
    app_model: Rc<AppModel>,
    dispatcher: Box<dyn ActionDispatcher>,
    command_sender: UnboundedSender<Command>,
    connect_command_sender: UnboundedSender<ConnectCommand>,
}

impl PlayerNotifier {
    pub fn new(
        app_model: Rc<AppModel>,
        dispatcher: Box<dyn ActionDispatcher>,
        command_sender: UnboundedSender<Command>,
        connect_command_sender: UnboundedSender<ConnectCommand>,
    ) -> Self {
        Self {
            app_model,
            dispatcher,
            command_sender,
            connect_command_sender,
        }
    }

    fn is_playing(&self) -> bool {
        self.app_model.get_state().playback.is_playing()
    }

    fn currently_playing(&self) -> Option<CurrentlyPlaying> {
        let state = self.app_model.get_state();
        let song = state.playback.current_song_id()?;
        let offset = state.playback.current_song_index()?;
        let source = state.playback.current_source().cloned();
        let result = match source {
            Some(source) if source.has_spotify_uri() => CurrentlyPlaying::WithSource {
                source,
                offset,
                song,
            },
            _ => CurrentlyPlaying::Songs {
                songs: state.playback.songs().map_collect(|s| s.id),
                offset,
            },
        };
        Some(result)
    }

    fn device(&self) -> impl Deref<Target = Device> + '_ {
        self.app_model.map_state(|s| s.playback.current_device())
    }

    fn notify_login(&self, event: &LoginEvent) {
        info!("notify_login: {:?}", event);
        let command = match event {
            LoginEvent::LoginStarted(LoginStartedEvent::Restore) => Some(Command::Restore),
            LoginEvent::LoginStarted(LoginStartedEvent::InitLogin) => Some(Command::InitLogin),
            LoginEvent::LoginStarted(LoginStartedEvent::CompleteLogin) => {
                Some(Command::CompleteLogin)
            }
            LoginEvent::FreshTokenRequested => Some(Command::RefreshToken),
            LoginEvent::LogoutCompleted => Some(Command::Logout),
            _ => None,
        };

        if let Some(command) = command {
            self.send_command_to_local_player(command);
        }
    }

    fn notify_connect_player(&self, event: &PlaybackEvent) {
        let event = event.clone();
        let currently_playing = self.currently_playing();
        let command = match event {
            PlaybackEvent::TrackChanged(_) | PlaybackEvent::SourceChanged => {
                match currently_playing {
                    Some(CurrentlyPlaying::WithSource {
                        source,
                        offset,
                        song,
                    }) => Some(ConnectCommand::PlayerLoadInContext {
                        source,
                        offset,
                        song,
                    }),
                    Some(CurrentlyPlaying::Songs { songs, offset }) => {
                        Some(ConnectCommand::PlayerLoad { songs, offset })
                    }
                    None => None,
                }
            }
            PlaybackEvent::TrackSeeked(position) => {
                Some(ConnectCommand::PlayerSeek(position as usize))
            }
            PlaybackEvent::PlaybackPaused => Some(ConnectCommand::PlayerPause),
            PlaybackEvent::PlaybackResumed => Some(ConnectCommand::PlayerResume),
            PlaybackEvent::VolumeSet(volume) => Some(ConnectCommand::PlayerSetVolume(
                (volume * 100f64).trunc() as u8,
            )),
            PlaybackEvent::RepeatModeChanged(mode) => Some(ConnectCommand::PlayerRepeat(mode)),
            PlaybackEvent::ShuffleChanged(shuffled) => {
                Some(ConnectCommand::PlayerShuffle(shuffled))
            }
            _ => None,
        };

        if let Some(command) = command {
            self.send_command_to_connect_player(command);
        }
    }

    fn notify_local_player(&self, event: &PlaybackEvent) {
        let command = match event {
            PlaybackEvent::PlaybackPaused => Some(Command::PlayerPause),
            PlaybackEvent::PlaybackResumed => Some(Command::PlayerResume),
            PlaybackEvent::PlaybackStopped => Some(Command::PlayerStop),
            PlaybackEvent::VolumeSet(volume) => Some(Command::PlayerSetVolume(*volume)),
            PlaybackEvent::TrackChanged(id) => {
                info!("track changed: {}", id);
                SpotifyId::from_base62(id).ok().map(|mut track| {
                    track.item_type = SpotifyItemType::Track;
                    Command::PlayerLoad {
                        track,
                        resume: true,
                    }
                })
            }
            PlaybackEvent::SourceChanged => {
                let resume = self.is_playing();
                self.currently_playing()
                    .and_then(|c| SpotifyId::from_base62(c.song_id()).ok())
                    .map(|mut track| {
                        track.item_type = SpotifyItemType::Track;
                        Command::PlayerLoad { track, resume }
                    })
            }
            PlaybackEvent::TrackSeeked(position) => Some(Command::PlayerSeek(*position)),
            PlaybackEvent::Preload(id) => SpotifyId::from_base62(id)
                .ok()
                .map(|mut track| {
                    track.item_type = SpotifyItemType::Track;
                    track
                })
                .map(Command::PlayerPreload),
            _ => None,
        };

        if let Some(command) = command {
            self.send_command_to_local_player(command);
        }
    }

    fn send_command_to_connect_player(&self, command: ConnectCommand) {
        self.connect_command_sender.unbounded_send(command).unwrap();
    }

    fn send_command_to_local_player(&self, command: Command) {
        let dispatcher = &self.dispatcher;
        self.command_sender
            .unbounded_send(command)
            .unwrap_or_else(|_| {
                dispatcher.dispatch(AppAction::LoginAction(LoginAction::SetLoginFailure));
            });
    }

    fn switch_device(&mut self, device: &Device) {
        match device {
            Device::Connect(device) => {
                self.send_command_to_local_player(Command::PlayerStop);
                self.send_command_to_connect_player(ConnectCommand::SetDevice(device.id.clone()));
                self.notify_connect_player(&PlaybackEvent::SourceChanged);
            }
            Device::Local => {
                self.send_command_to_connect_player(ConnectCommand::PlayerStop);
                self.notify_local_player(&PlaybackEvent::SourceChanged);
            }
        }
    }
}

impl EventListener for PlayerNotifier {
    fn on_event(&mut self, event: &AppEvent) {
        let device = self.device().clone();
        match (device, event) {
            (_, AppEvent::LoginEvent(event)) => self.notify_login(event),
            (_, AppEvent::PlaybackEvent(PlaybackEvent::SwitchedDevice(d))) => self.switch_device(d),
            (Device::Local, AppEvent::PlaybackEvent(event)) => self.notify_local_player(event),
            (Device::Local, AppEvent::SettingsEvent(SettingsEvent::PlayerSettingsChanged)) => {
                self.send_command_to_local_player(Command::ReloadSettings)
            }
            (Device::Connect(_), AppEvent::PlaybackEvent(event)) => {
                self.notify_connect_player(event)
            }
            _ => {}
        }
    }
}
