#[macro_use(clone)]
extern crate glib;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate gettextrs;

use app::state::ScreenName;
use futures::channel::mpsc::UnboundedSender;
use gettextrs::*;
use gio::prelude::*;
use gio::ApplicationFlags;
use gio::SimpleAction;
use gtk::prelude::*;

mod api;
mod app;
mod config;
mod connect;
mod dbus;
mod player;
mod settings;

use crate::app::components::expose_custom_widgets;
use crate::app::dispatch::{spawn_task_handler, DispatchLoop};
use crate::app::{state::PlaybackAction, App, AppAction, BrowserAction};

fn main() {
    let settings = settings::SpotSettings::new_from_gsettings().unwrap_or_default();
    setup_gtk(&settings);

    // Looks like there's a side effect to declaring widgets that allows them to be referenced them in ui/blueprint files
    // so here goes!
    expose_custom_widgets();

    let gtk_app = gtk::Application::new(Some(config::APPID), ApplicationFlags::HANDLES_OPEN);
    let builder = gtk::Builder::from_resource("/dev/alextren/Spot/window.ui");
    let window: libadwaita::ApplicationWindow = builder.object("window").unwrap();

    // In debug mode, the app id is different (see meson config) so we fix the resource path (and add a distinctive style)
    // Having a different app id allows running both the stable and development version at the same time
    if cfg!(debug_assertions) {
        // window.add_css_class("devel");
        gtk_app.set_resource_base_path(Some("/dev/alextren/Spot"));
    }

    let context = glib::MainContext::default();
    let dispatch_loop = DispatchLoop::new();
    let sender = dispatch_loop.make_dispatcher();

    // Couple of actions used with shortcuts
    register_actions(&gtk_app, sender.clone());
    setup_credits(builder.object::<libadwaita::AboutDialog>("about").unwrap());

    // Main app logic is hooked up here
    let app = App::new(
        settings,
        builder,
        sender.clone(),
        spawn_task_handler(&context),
    );
    context.spawn_local(app.attach(dispatch_loop));

    let sender_clone = sender.clone();
    gtk_app.connect_activate(move |gtk_app| {
        debug!("activate");
        if let Some(existing_window) = gtk_app.active_window() {
            existing_window.present();
        } else {
            // Only send the Start action if we've just created the window
            window.set_application(Some(gtk_app));
            gtk_app.add_window(&window);
            sender_clone.unbounded_send(AppAction::Start).unwrap();
        }
    });

    gtk_app.connect_open(move |gtk_app, targets, _| {
        gtk_app.activate();

        // There should only be one target because %u is used in desktop file
        let target = &targets[0];
        let uri = target.uri().to_string();
        let action = AppAction::OpenURI(uri)
            .unwrap_or_else(|| AppAction::ShowNotification(gettext("Failed to open link!")));
        sender.unbounded_send(action).unwrap();
    });

    context.invoke_local(move || {
        gtk_app.run();
    });

    std::process::exit(0);
}

fn setup_gtk(settings: &settings::SpotSettings) {
    // Setup logging
    env_logger::init();

    // Setup translations
    textdomain("spot")
        .and_then(|_| bindtextdomain("spot", config::LOCALEDIR))
        .and_then(|_| bind_textdomain_codeset("spot", "UTF-8"))
        .expect("Could not setup localization");

    // Setup Gtk, Adwaita...
    gtk::init().unwrap_or_else(|_| panic!("Failed to initialize GTK"));
    libadwaita::init().unwrap_or_else(|_| panic!("Failed to initialize libadwaita"));

    let manager = libadwaita::StyleManager::default();
    manager.set_color_scheme(settings.theme_preference);

    let res = gio::Resource::load(config::PKGDATADIR.to_owned() + "/spot.gresource")
        .expect("Could not load resources");
    gio::resources_register(&res);

    let provider = gtk::CssProvider::new();
    provider.load_from_resource("/dev/alextren/Spot/app.css");

    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn setup_credits(about: libadwaita::AboutDialog) {
    // Read from a couple files at compile time and update the about dialog
    let authors: Vec<&str> = include_str!("../AUTHORS")
        .trim_end_matches('\n')
        .split('\n')
        .collect();
    let translators = include_str!("../TRANSLATORS").trim_end_matches('\n');
    let artists: Vec<&str> = include_str!("../ARTISTS")
        .trim_end_matches('\n')
        .split('\n')
        .collect();
    about.set_version(config::VERSION);
    about.set_developers(&authors);
    about.set_translator_credits(translators);
    about.set_artists(&artists);
}

fn register_actions(app: &gtk::Application, sender: UnboundedSender<AppAction>) {
    let quit = SimpleAction::new("quit", None);
    quit.connect_activate(clone!(
        #[weak]
        app,
        move |_, _| {
            if let Some(existing_window) = app.active_window() {
                existing_window.close();
            }
            app.quit();
        }
    ));
    app.add_action(&quit);

    app.add_action(&make_action(
        "toggle_playback",
        PlaybackAction::TogglePlay.into(),
        sender.clone(),
    ));

    app.add_action(&make_action(
        "player_prev",
        PlaybackAction::Previous.into(),
        sender.clone(),
    ));

    app.add_action(&make_action(
        "player_next",
        PlaybackAction::Next.into(),
        sender.clone(),
    ));

    app.add_action(&make_action(
        "nav_pop",
        AppAction::BrowserAction(BrowserAction::NavigationPop),
        sender.clone(),
    ));

    app.add_action(&make_action(
        "search",
        AppAction::BrowserAction(BrowserAction::NavigationPush(ScreenName::Search)),
        sender.clone(),
    ));

    app.add_action(&{
        let action = SimpleAction::new("open_playlist", Some(glib::VariantTy::STRING));
        action.set_enabled(true);
        action.connect_activate(move |_, playlist_id| {
            if let Some(id) = playlist_id.and_then(|s| s.str()) {
                sender
                    .unbounded_send(AppAction::ViewPlaylist(id.to_owned()))
                    .unwrap();
            }
        });
        action
    });
}

fn make_action(
    name: &str,
    app_action: AppAction,
    sender: UnboundedSender<AppAction>,
) -> SimpleAction {
    let action = SimpleAction::new(name, None);
    action.connect_activate(move |_, _| {
        sender.unbounded_send(app_action.clone()).unwrap();
    });
    action
}
