//! The two views as surfaces of the shell.
//!
//! Pattern: Adapter — `Surface` (the shell's trait) over the handle of a component; they hold no state of their own.

use fs_components_app::Surface;
use fs_components_kanban::KanbanBoard;
use fs_components_timeline::Timeline;
use fs_components_widgets::Element;

/// The status board as a screen.
pub struct BoardSurface(pub KanbanBoard);

impl Surface for BoardSurface {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }
}

/// The timeline as a screen.
pub struct TimelineSurface(pub Timeline);

impl Surface for TimelineSurface {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }
}
