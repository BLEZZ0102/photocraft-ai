//! The app id every PhotoCraft OS window carries: the main window (`main.rs`), the runtime's
//! windows (`InstallSpec::app_id`) and the extra windows built with [`builder`], so docks show
//! the PhotoCraft icon on them and Wayland compositors' window rules can match them (Hyprland's
//! `class:`, #6697). Without one the app id is empty.

/// Windows beta windows have an independent identity. Other platforms retain their launcher id.
pub const APP_ID: &str = if cfg!(windows) { "io.github.blezz0102.photocraft-ai" } else { "ai.storyteller.photocraft" };

/// A builder for an extra OS window, carrying [`APP_ID`].
pub fn builder() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default().with_app_id(APP_ID)
}

#[cfg(test)]
mod tests {
    #[test]
    fn extra_windows_carry_the_app_id() {
        let expected = if cfg!(windows) { "io.github.blezz0102.photocraft-ai" } else { "ai.storyteller.photocraft" };
        assert_eq!(super::builder().app_id.as_deref(), Some(expected));
    }
}
