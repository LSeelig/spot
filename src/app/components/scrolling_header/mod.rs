mod scrolling_header_widget;
use gtk::prelude::StaticType;
pub use scrolling_header_widget::*;

pub fn expose_widgets() {
    scrolling_header_widget::ScrollingHeaderWidget::static_type();
}
