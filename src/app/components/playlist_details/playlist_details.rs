use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use std::rc::Rc;

use super::playlist_header::PlaylistHeaderWidget;
use super::playlist_headerbar::PlaylistHeaderBarWidget;
use super::PlaylistDetailsModel;

use crate::app::components::{
    Component, EventListener, Playlist, PlaylistModel, ScrollingHeaderWidget,
};
use crate::app::dispatch::Worker;
use crate::app::loader::ImageLoader;
use crate::app::state::{PlaybackEvent, SelectionEvent};
use crate::app::{AppEvent, BrowserEvent};
use libadwaita::subclass::prelude::BinImpl;

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/alextren/Spot/components/playlist_details.ui")]
    pub struct PlaylistDetailsWidget {
        #[template_child]
        pub headerbar: TemplateChild<PlaylistHeaderBarWidget>,

        #[template_child]
        pub scrolling_header: TemplateChild<ScrollingHeaderWidget>,

        #[template_child]
        pub header_widget: TemplateChild<PlaylistHeaderWidget>,

        #[template_child]
        pub tracks: TemplateChild<gtk::ListView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistDetailsWidget {
        const NAME: &'static str = "PlaylistDetailsWidget";
        type Type = super::PlaylistDetailsWidget;
        type ParentType = libadwaita::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaylistDetailsWidget {
        fn constructed(&self) {
            self.parent_constructed();
            self.header_widget.set_grows_automatically();
        }
    }

    impl WidgetImpl for PlaylistDetailsWidget {}
    impl BinImpl for PlaylistDetailsWidget {}
}

glib::wrapper! {
    pub struct PlaylistDetailsWidget(ObjectSubclass<imp::PlaylistDetailsWidget>) @extends gtk::Widget, libadwaita::Bin;
}

impl PlaylistDetailsWidget {
    fn new() -> Self {
        glib::Object::new()
    }

    fn playlist_tracks_widget(&self) -> &gtk::ListView {
        self.imp().tracks.as_ref()
    }

    fn connect_bottom_edge<F>(&self, f: F)
    where
        F: Fn() + 'static,
    {
        self.imp().scrolling_header.connect_bottom_edge(f);
    }

    fn set_header_visible(&self, visible: bool) {
        let widget = self.imp();
        widget.headerbar.set_title_visible(true);
        if visible {
            widget.headerbar.add_classes(&["flat"]);
        } else {
            widget.headerbar.remove_classes(&["flat"]);
        }
    }

    fn connect_header(&self) {
        self.set_header_visible(false);
        self.imp()
            .scrolling_header
            .connect_header_visibility(clone!(
                #[weak(rename_to = _self)]
                self,
                move |visible| {
                    _self.set_header_visible(visible);
                }
            ));
    }

    fn set_loaded(&self) {
        self.imp()
            .scrolling_header
            .add_css_class("container--loaded");
    }

    fn set_editing(&self, editing: bool) {
        self.imp().header_widget.set_editing(editing);
        self.imp().headerbar.set_editing(editing);
    }

    fn set_editable(&self, editing: bool) {
        self.imp().headerbar.set_editable(editing);
    }

    fn set_info(&self, playlist: &str, owner: &str) {
        self.imp().header_widget.set_info(playlist, owner);
        self.imp().headerbar.set_title(Some(playlist));
    }

    fn set_playing(&self, is_playing: bool) {
        self.imp().header_widget.set_playing(is_playing);
    }

    fn set_artwork(&self, art: &gdk_pixbuf::Pixbuf) {
        self.imp().header_widget.set_artwork(art);
    }

    fn connect_owner_clicked<F>(&self, f: F)
    where
        F: Fn() + 'static,
    {
        self.imp().header_widget.connect_owner_clicked(f);
    }

    pub fn connect_edit<F>(&self, f: F)
    where
        F: Fn() + 'static,
    {
        self.imp().headerbar.connect_edit(f);
    }

    pub fn connect_cancel<F>(&self, f: F)
    where
        F: Fn() + 'static,
    {
        self.imp().headerbar.connect_cancel(clone!(
            #[weak(rename_to = _self)]
            self,
            move || {
                _self.imp().header_widget.reset_playlist_name();
                f();
            }
        ));
    }

    pub fn connect_play<F>(&self, f: F)
    where
        F: Fn() + 'static,
    {
        self.imp().header_widget.connect_play(f);
    }

    pub fn connect_done<F>(&self, f: F)
    where
        F: Fn(String) + 'static,
    {
        self.imp().headerbar.connect_ok(clone!(
            #[weak(rename_to = _self)]
            self,
            move || {
                let s = _self.imp().header_widget.get_edited_playlist_name();
                f(s);
            }
        ));
    }

    pub fn connect_go_back<F>(&self, f: F)
    where
        F: Fn() + 'static,
    {
        self.imp().headerbar.connect_go_back(f);
    }
}

pub struct PlaylistDetails {
    model: Rc<PlaylistDetailsModel>,
    worker: Worker,
    widget: PlaylistDetailsWidget,
    children: Vec<Box<dyn EventListener>>,
}

impl PlaylistDetails {
    pub fn new(model: Rc<PlaylistDetailsModel>, worker: Worker) -> Self {
        if model.get_playlist_info().is_none() {
            model.load_playlist_info();
        }

        let widget = PlaylistDetailsWidget::new();
        let playlist = Box::new(Playlist::new(
            widget.playlist_tracks_widget().clone(),
            model.clone(),
            worker.clone(),
        ));

        widget.set_editable(model.is_playlist_editable());

        widget.connect_header();

        widget.connect_bottom_edge(clone!(
            #[weak]
            model,
            move || {
                model.load_more_tracks();
            }
        ));

        widget.connect_owner_clicked(clone!(
            #[weak]
            model,
            move || model.view_owner()
        ));

        widget.connect_edit(clone!(
            #[weak]
            model,
            move || {
                model.enable_selection();
            }
        ));

        widget.connect_cancel(clone!(
            #[weak]
            model,
            move || model.disable_selection()
        ));
        widget.connect_done(clone!(
            #[weak]
            model,
            move |n| {
                model.disable_selection();
                model.update_playlist_details(n);
            }
        ));

        widget.connect_play(clone!(
            #[weak]
            model,
            move || model.toggle_play_playlist()
        ));

        widget.connect_go_back(clone!(
            #[weak]
            model,
            move || model.go_back()
        ));

        Self {
            model,
            worker,
            widget,
            children: vec![playlist],
        }
    }

    fn update_details(&self) {
        if let Some(info) = self.model.get_playlist_info() {
            let title = &info.title[..];
            let owner = &info.owner.display_name[..];
            let art_url = info.art.as_ref();

            self.widget.set_info(title, owner);

            if let Some(art_url) = art_url.cloned() {
                let widget = self.widget.downgrade();
                self.worker.send_local_task(async move {
                    let pixbuf = ImageLoader::new()
                        .load_remote(&art_url[..], "jpg", 320, 320)
                        .await;
                    if let (Some(widget), Some(ref pixbuf)) = (widget.upgrade(), pixbuf) {
                        widget.set_artwork(pixbuf);
                        widget.set_loaded();
                    }
                });
            } else {
                self.widget.set_loaded();
            }
        }
    }

    fn update_playing(&self, is_playing: bool) {
        if !self.model.playlist_is_playing() || !self.model.is_playing() {
            self.widget.set_playing(false);
            return;
        }
        self.widget.set_playing(is_playing);
    }

    fn set_editing(&self, editable: bool) {
        if !self.model.is_playlist_editable() {
            return;
        }
        self.widget.set_editing(editable);
    }
}

impl Component for PlaylistDetails {
    fn get_root_widget(&self) -> &gtk::Widget {
        self.widget.upcast_ref()
    }

    fn get_children(&mut self) -> Option<&mut Vec<Box<dyn EventListener>>> {
        Some(&mut self.children)
    }
}

impl EventListener for PlaylistDetails {
    fn on_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::BrowserEvent(BrowserEvent::PlaylistDetailsLoaded(id))
                if id == &self.model.id =>
            {
                self.update_details();
                self.update_playing(true);
            }
            AppEvent::SelectionEvent(SelectionEvent::SelectionModeChanged(editing)) => {
                self.set_editing(*editing);
            }
            AppEvent::PlaybackEvent(PlaybackEvent::PlaybackPaused) => {
                self.update_playing(false);
            }
            AppEvent::PlaybackEvent(PlaybackEvent::PlaybackResumed) => {
                self.update_playing(true);
            }
            _ => {}
        }
        self.broadcast_event(event);
    }
}
