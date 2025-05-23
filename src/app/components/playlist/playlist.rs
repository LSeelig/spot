use gio::prelude::*;
use gtk::prelude::*;
use std::ops::Deref;
use std::rc::Rc;

use crate::app::components::utils::{ancestor, AnimatorDefault};
use crate::app::components::{Component, EventListener, SongWidget};
use crate::app::models::{SongListModel, SongModel, SongState};
use crate::app::state::{PlaybackEvent, SelectionEvent, SelectionState};
use crate::app::{AppEvent, Worker};

pub trait PlaylistModel {
    fn is_paused(&self) -> bool;

    fn song_list_model(&self) -> SongListModel;

    fn current_song_id(&self) -> Option<String>;

    fn play_song_at(&self, pos: usize, id: &str);

    fn autoscroll_to_playing(&self) -> bool {
        true
    }

    fn show_song_covers(&self) -> bool {
        true
    }

    fn actions_for(&self, _id: &str) -> Option<gio::ActionGroup> {
        None
    }
    fn menu_for(&self, _id: &str) -> Option<gio::MenuModel> {
        None
    }

    fn select_song(&self, _id: &str) {}
    fn deselect_song(&self, _id: &str) {}
    fn enable_selection(&self) -> bool {
        false
    }

    fn selection(&self) -> Option<Box<dyn Deref<Target = SelectionState> + '_>> {
        None
    }

    fn is_selection_enabled(&self) -> bool {
        self.selection()
            .map(|s| s.is_selection_enabled())
            .unwrap_or(false)
    }

    fn song_state(&self, id: &str) -> SongState {
        let is_playing = self.current_song_id().map(|s| s.eq(id)).unwrap_or(false);
        let is_selected = self
            .selection()
            .map(|s| s.is_song_selected(id))
            .unwrap_or(false);
        SongState {
            is_selected,
            is_playing,
        }
    }

    fn toggle_select(&self, id: &str) {
        if let Some(selection) = self.selection() {
            if selection.is_song_selected(id) {
                self.deselect_song(id);
            } else {
                self.select_song(id);
            }
        }
    }
}

pub struct Playlist<Model> {
    animator: AnimatorDefault,
    listview: gtk::ListView,
    model: Rc<Model>,
}

impl<Model> Playlist<Model>
where
    Model: PlaylistModel + 'static,
{
    pub fn new(listview: gtk::ListView, model: Rc<Model>, worker: Worker) -> Self {
        let list_model = model.song_list_model();
        let selection_model = gtk::NoSelection::new(Some(list_model.clone()));
        let factory = gtk::SignalListItemFactory::new();

        listview.add_css_class("playlist");
        listview.set_show_separators(true);
        listview.set_valign(gtk::Align::Start);

        listview.set_factory(Some(&factory));
        listview.set_single_click_activate(true);
        listview.set_model(Some(&selection_model));
        Self::set_paused(&listview, model.is_paused());
        Self::set_selection_active(&listview, model.is_selection_enabled());

        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            item.set_child(Some(&SongWidget::new()));
        });

        factory.connect_bind(clone!(
            #[weak]
            model,
            move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                let song_model = item.item().unwrap().downcast::<SongModel>().unwrap();
                song_model.set_state(model.song_state(&song_model.get_id()));

                let widget = item.child().unwrap().downcast::<SongWidget>().unwrap();
                widget.bind(&song_model, worker.clone(), model.show_song_covers());

                let id = &song_model.get_id();
                widget.set_actions(model.actions_for(id).as_ref());
                widget.set_menu(model.menu_for(id).as_ref());
            }
        ));

        factory.connect_unbind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let song_model = item.item().unwrap().downcast::<SongModel>().unwrap();
            song_model.unbind_all();
        });

        listview.connect_activate(clone!(
            #[weak]
            list_model,
            #[weak]
            model,
            move |_, position| {
                let song = list_model
                    .index_continuous(position as usize)
                    .expect("attempt to access invalid index");
                let song = song.description();
                let selection_enabled = model.is_selection_enabled();
                if selection_enabled {
                    model.toggle_select(&song.id);
                } else {
                    model.play_song_at(position as usize, &song.id);
                }
            }
        ));

        let press_gesture = gtk::GestureLongPress::new();
        press_gesture.set_touch_only(false);
        press_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        press_gesture.connect_pressed(clone!(
            #[weak]
            model,
            move |_, _, _| {
                model.enable_selection();
            }
        ));
        listview.add_controller(press_gesture);

        Self {
            animator: AnimatorDefault::ease_in_out_animator(),
            listview,
            model,
        }
    }

    fn autoscroll_to_playing(&self, index: usize) {
        let len = self.model.song_list_model().partial_len() as f64;
        let scrolled_window: Option<gtk::ScrolledWindow> = ancestor(&self.listview);
        let adj = scrolled_window.map(|w| w.vadjustment());
        if let Some(adj) = adj {
            let v = adj.value();
            let v2 = v + 0.9 * adj.page_size();
            let pos = (index as f64) * adj.upper() / len;
            debug!("estimated pos: {}", pos);
            debug!("current window: {} -- {}", v, v2);
            if pos < v || pos > v2 {
                self.animator.animate(
                    20,
                    clone!(
                        #[weak]
                        adj,
                        #[upgrade_or]
                        false,
                        move |p| {
                            let v = adj.value();
                            adj.set_value(v + p * (pos - v));
                            true
                        }
                    ),
                );
            }
        }
    }

    fn update_list(&self) {
        let autoscroll_to_playing = self.model.autoscroll_to_playing();
        let is_selection_enabled = self.model.is_selection_enabled();

        self.model.song_list_model().for_each(|i, model_song| {
            let state = self.model.song_state(&model_song.get_id());
            model_song.set_state(state);
            if state.is_playing && autoscroll_to_playing && !is_selection_enabled {
                self.autoscroll_to_playing(i);
            }
        });
    }

    fn set_selection_active(listview: &gtk::ListView, active: bool) {
        let class_name = "playlist--selectable";
        if active {
            listview.add_css_class(class_name);
        } else {
            listview.remove_css_class(class_name);
        }
    }

    fn set_paused(listview: &gtk::ListView, paused: bool) {
        let class_name = "playlist--paused";
        if paused {
            listview.add_css_class(class_name);
        } else {
            listview.remove_css_class(class_name);
        }
    }
}

impl SongModel {
    fn set_state(
        &self,
        SongState {
            is_playing,
            is_selected,
        }: SongState,
    ) {
        self.set_playing(is_playing);
        self.set_selected(is_selected);
    }
}

impl<Model> EventListener for Playlist<Model>
where
    Model: PlaylistModel + 'static,
{
    fn on_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::SelectionEvent(SelectionEvent::SelectionChanged) => {
                self.update_list();
            }
            AppEvent::PlaybackEvent(PlaybackEvent::TrackChanged(_)) => {
                self.update_list();
            }
            AppEvent::PlaybackEvent(
                PlaybackEvent::PlaybackResumed | PlaybackEvent::PlaybackPaused,
            ) => {
                Self::set_paused(&self.listview, self.model.is_paused());
            }
            AppEvent::SelectionEvent(SelectionEvent::SelectionModeChanged(_)) => {
                Self::set_selection_active(&self.listview, self.model.is_selection_enabled());
                self.update_list();
            }
            _ => {}
        }
    }
}

impl<Model> Component for Playlist<Model> {
    fn get_root_widget(&self) -> &gtk::Widget {
        self.listview.upcast_ref()
    }
}
