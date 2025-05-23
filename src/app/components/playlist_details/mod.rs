#[allow(clippy::module_inception)]
mod playlist_details;
mod playlist_details_model;
mod playlist_header;
mod playlist_headerbar;

pub use playlist_details::*;
pub use playlist_details_model::*;

use gtk::prelude::StaticType;

pub fn expose_widgets() {
    playlist_headerbar::PlaylistHeaderBarWidget::static_type();
}
