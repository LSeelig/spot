use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use std::rc::Rc;

use super::SavedTracksModel;
use crate::app::components::{Component, EventListener, Playlist};
use crate::app::state::LoginEvent;
use crate::app::{AppEvent, Worker};
use libadwaita::subclass::prelude::BinImpl;

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/alextren/Spot/components/saved_tracks.ui")]
    pub struct SavedTracksWidget {
        #[template_child]
        pub song_list: TemplateChild<gtk::ListView>,

        #[template_child]
        pub scrolled_window: TemplateChild<gtk::ScrolledWindow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SavedTracksWidget {
        const NAME: &'static str = "SavedTracksWidget";
        type Type = super::SavedTracksWidget;
        type ParentType = libadwaita::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SavedTracksWidget {}
    impl WidgetImpl for SavedTracksWidget {}
    impl BinImpl for SavedTracksWidget {}
}

glib::wrapper! {
    pub struct SavedTracksWidget(ObjectSubclass<imp::SavedTracksWidget>) @extends gtk::Widget, libadwaita::Bin;
}

impl SavedTracksWidget {
    fn new() -> Self {
        glib::Object::new()
    }

    fn connect_bottom_edge<F>(&self, f: F)
    where
        F: Fn() + 'static,
    {
        self.imp()
            .scrolled_window
            .connect_edge_reached(move |_, pos| {
                if let gtk::PositionType::Bottom = pos {
                    f()
                }
            });
    }

    fn song_list_widget(&self) -> &gtk::ListView {
        self.imp().song_list.as_ref()
    }
}

pub struct SavedTracks {
    widget: SavedTracksWidget,
    model: Rc<SavedTracksModel>,
    children: Vec<Box<dyn EventListener>>,
}

impl SavedTracks {
    pub fn new(model: Rc<SavedTracksModel>, worker: Worker) -> Self {
        let widget = SavedTracksWidget::new();

        widget.connect_bottom_edge(clone!(
            #[weak]
            model,
            move || {
                model.load_more();
            }
        ));

        let playlist = Playlist::new(widget.song_list_widget().clone(), model.clone(), worker);

        Self {
            widget,
            model,
            children: vec![Box::new(playlist)],
        }
    }
}

impl Component for SavedTracks {
    fn get_root_widget(&self) -> &gtk::Widget {
        self.widget.upcast_ref()
    }

    fn get_children(&mut self) -> Option<&mut Vec<Box<dyn EventListener>>> {
        Some(&mut self.children)
    }
}

impl EventListener for SavedTracks {
    fn on_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Started | AppEvent::LoginEvent(LoginEvent::LoginCompleted) => {
                self.model.load_initial();
            }
            _ => {}
        }
        self.broadcast_event(event);
    }
}
