use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use libadwaita::subclass::prelude::*;

use crate::app::components::labels;

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/alextren/Spot/components/release_details.ui")]
    pub struct ReleaseDetailsDialog {
        #[template_child]
        pub album_artist: TemplateChild<libadwaita::WindowTitle>,

        #[template_child]
        pub label: TemplateChild<gtk::Label>,

        #[template_child]
        pub release: TemplateChild<gtk::Label>,

        #[template_child]
        pub tracks: TemplateChild<gtk::Label>,

        #[template_child]
        pub copyright: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ReleaseDetailsDialog {
        const NAME: &'static str = "ReleaseDetailsDialog";
        type Type = super::ReleaseDetailsDialog;
        type ParentType = libadwaita::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ReleaseDetailsDialog {}
    impl WidgetImpl for ReleaseDetailsDialog {}
    impl AdwDialogImpl for ReleaseDetailsDialog {}
}

glib::wrapper! {
    pub struct
    ReleaseDetailsDialog(ObjectSubclass<imp::ReleaseDetailsDialog>) @extends gtk::Widget, libadwaita::Dialog;
}

impl ReleaseDetailsDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_details(
        &self,
        album: &str,
        artist: &str,
        label: &str,
        release_date: &str,
        track_count: usize,
        copyright: &str,
    ) {
        let widget = self.imp();

        widget
            .album_artist
            .set_title(&labels::album_by_artist_label(album, artist));

        widget.label.set_text(label);
        widget.release.set_text(release_date);
        widget.tracks.set_text(&track_count.to_string());
        widget.copyright.set_text(copyright);
    }
}
