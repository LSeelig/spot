use crate::app::components::EventListener;
use crate::app::AppEvent;
use crate::settings::SpotSettings;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use libadwaita::prelude::*;

use super::SettingsModel;

const SETTINGS: &str = "dev.alextren.Spot";

mod imp {

    use super::*;
    use libadwaita::subclass::prelude::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/alextren/Spot/components/settings.ui")]
    pub struct SettingsDialog {
        #[template_child]
        pub player_bitrate: TemplateChild<libadwaita::ComboRow>,

        #[template_child]
        pub alsa_device: TemplateChild<gtk::Entry>,

        #[template_child]
        pub alsa_device_row: TemplateChild<libadwaita::ActionRow>,

        #[template_child]
        pub audio_backend: TemplateChild<libadwaita::ComboRow>,

        #[template_child]
        pub gapless_playback: TemplateChild<libadwaita::ActionRow>,

        #[template_child]
        pub ap_port: TemplateChild<gtk::Entry>,

        #[template_child]
        pub theme: TemplateChild<libadwaita::ComboRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SettingsDialog {
        const NAME: &'static str = "SettingsWindow";
        type Type = super::SettingsDialog;
        type ParentType = libadwaita::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SettingsDialog {}
    impl WidgetImpl for SettingsDialog {}
    impl AdwDialogImpl for SettingsDialog {}
    impl PreferencesDialogImpl for SettingsDialog {}
}

glib::wrapper! {
    pub struct SettingsDialog(ObjectSubclass<imp::SettingsDialog>) @extends gtk::Widget, libadwaita::Dialog, libadwaita::PreferencesDialog;
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsDialog {
    pub fn new() -> Self {
        let dialog: Self = glib::Object::new();

        dialog.bind_backend_and_device();
        dialog.bind_settings();
        dialog.connect_theme_select();
        dialog
    }

    fn bind_backend_and_device(&self) {
        let widget = self.imp();

        let audio_backend = widget
            .audio_backend
            .downcast_ref::<libadwaita::ComboRow>()
            .unwrap();
        let alsa_device_row = widget
            .alsa_device_row
            .downcast_ref::<libadwaita::ActionRow>()
            .unwrap();

        audio_backend
            .bind_property("selected", alsa_device_row, "visible")
            .transform_to(|_, value: u32| Some(value == 1))
            .build();

        if audio_backend.selected() == 0 {
            alsa_device_row.set_visible(false);
        }
    }

    fn bind_settings(&self) {
        let widget = self.imp();
        let settings = gio::Settings::new(SETTINGS);

        let player_bitrate = widget
            .player_bitrate
            .downcast_ref::<libadwaita::ComboRow>()
            .unwrap();
        settings
            .bind("player-bitrate", player_bitrate, "selected")
            .mapping(|variant, _| {
                variant.str().map(|s| {
                    match s {
                        "96" => 0,
                        "160" => 1,
                        "320" => 2,
                        _ => unreachable!(),
                    }
                    .to_value()
                })
            })
            .set_mapping(|value, _| {
                value.get::<u32>().ok().map(|u| {
                    match u {
                        0 => "96",
                        1 => "160",
                        2 => "320",
                        _ => unreachable!(),
                    }
                    .to_variant()
                })
            })
            .build();

        let alsa_device = widget.alsa_device.downcast_ref::<gtk::Entry>().unwrap();
        settings.bind("alsa-device", alsa_device, "text").build();

        let audio_backend = widget
            .audio_backend
            .downcast_ref::<libadwaita::ComboRow>()
            .unwrap();
        settings
            .bind("audio-backend", audio_backend, "selected")
            .mapping(|variant, _| {
                variant.str().map(|s| {
                    match s {
                        "pulseaudio" => 0,
                        "alsa" => 1,
                        "gstreamer" => 2,
                        _ => unreachable!(),
                    }
                    .to_value()
                })
            })
            .set_mapping(|value, _| {
                value.get::<u32>().ok().map(|u| {
                    match u {
                        0 => "pulseaudio",
                        1 => "alsa",
                        2 => "gstreamer",
                        _ => unreachable!(),
                    }
                    .to_variant()
                })
            })
            .build();

        let gapless_playback = widget
            .gapless_playback
            .downcast_ref::<libadwaita::ActionRow>()
            .unwrap();
        settings
            .bind(
                "gapless-playback",
                &gapless_playback.activatable_widget().unwrap(),
                "active",
            )
            .build();

        let ap_port = widget.ap_port.downcast_ref::<gtk::Entry>().unwrap();
        settings
            .bind("ap-port", ap_port, "text")
            .mapping(|variant, _| variant.get::<u32>().map(|s| s.to_value()))
            .set_mapping(|value, _| value.get::<u32>().ok().map(|u| u.to_variant()))
            .build();

        let theme = widget.theme.downcast_ref::<libadwaita::ComboRow>().unwrap();
        settings
            .bind("theme-preference", theme, "selected")
            .mapping(|variant, _| {
                variant.str().map(|s| {
                    match s {
                        "light" => 0,
                        "dark" => 1,
                        "system" => 2,
                        _ => unreachable!(),
                    }
                    .to_value()
                })
            })
            .set_mapping(|value, _| {
                value.get::<u32>().ok().map(|u| {
                    match u {
                        0 => "light",
                        1 => "dark",
                        2 => "system",
                        _ => unreachable!(),
                    }
                    .to_variant()
                })
            })
            .build();
    }

    fn connect_theme_select(&self) {
        let widget = self.imp();
        let theme = widget.theme.downcast_ref::<libadwaita::ComboRow>().unwrap();
        theme.connect_selected_notify(|theme| {
            debug!("Theme switched! --> value: {}", theme.selected());
            let manager = libadwaita::StyleManager::default();

            let pref = match theme.selected() {
                0 => libadwaita::ColorScheme::ForceLight,
                1 => libadwaita::ColorScheme::ForceDark,
                _ => libadwaita::ColorScheme::Default,
            };

            manager.set_color_scheme(pref);
        });
    }

    fn connect_close<F>(&self, on_close: F)
    where
        F: Fn() + 'static,
    {
        let dialog = self.upcast_ref::<libadwaita::Dialog>();
        dialog.connect_close_attempt(move |_| {
            on_close();
        });
    }
}

pub struct Settings {
    parent: gtk::Window,
    settings_dialog: SettingsDialog,
}

impl Settings {
    pub fn new(parent: gtk::Window, model: SettingsModel) -> Self {
        let settings_dialog = SettingsDialog::new();

        settings_dialog.connect_close(move || {
            let new_settings = SpotSettings::new_from_gsettings().unwrap_or_default();
            if model.settings().player_settings != new_settings.player_settings {
                model.stop_player();
            }
            model.set_settings();
        });

        Self {
            parent,
            settings_dialog,
        }
    }

    fn dialog(&self) -> &libadwaita::Dialog {
        self.settings_dialog.upcast_ref::<libadwaita::Dialog>()
    }

    pub fn show_self(&self) {
        self.dialog().present(Some(&self.parent));
    }
}

impl EventListener for Settings {
    fn on_event(&mut self, _: &AppEvent) {}
}
