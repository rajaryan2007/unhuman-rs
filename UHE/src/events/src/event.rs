use crate::{KeyCode, MouseButton};

/// One thing that happened, in the order the platform reported it.
///
/// One enum carries every payload, rather than a class per event plus a tag to
/// dispatch on.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    WindowResized {
        width: u32,
        height: u32,
    },
    WindowCloseRequested,
    /// Focus gained (`true`) or lost (`false`).
    WindowFocused(bool),
    /// `repeat` is an auto-repeat.
    KeyPressed {
        key: KeyCode,
        repeat: bool,
    },
    KeyReleased {
        key: KeyCode,
    },
    /// The characters a key produced, separate from the press that produced
    /// them.
    TextInput(String),
    MouseButtonPressed(MouseButton),
    MouseButtonReleased(MouseButton),
    MouseMoved {
        x: f32,
        y: f32,
    },
    MouseScrolled {
        dx: f32,
        dy: f32,
    },
    /// Bracket the frame. Ordinary events: same queue, same subscribers, same
    /// order as everything else.
    FrameBegin,
    FrameEnd,
}
