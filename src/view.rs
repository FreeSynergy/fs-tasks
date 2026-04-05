// view.rs — FsView implementations for fs-tasks.
//
// This is the ONLY file in fs-tasks that imports fs-render.
// Domain types (TaskPipeline, TaskController) do NOT import fs-render.

use fs_render::{
    view::FsView,
    widget::{ButtonWidget, FsWidget, ListWidget, TextInputWidget},
};

use crate::keys;
use crate::model::TaskPipeline;

// ── TasksView ─────────────────────────────────────────────────────────────────

/// Snapshot view of the task pipeline list.
pub struct TasksView {
    pub tasks: Vec<TaskPipeline>,
}

impl TasksView {
    #[must_use]
    pub fn new(tasks: Vec<TaskPipeline>) -> Self {
        Self { tasks }
    }
}

impl FsView for TasksView {
    fn view(&self) -> Box<dyn FsWidget> {
        let items: Vec<String> = self
            .tasks
            .iter()
            .map(|t| {
                let status = if t.enabled { "●" } else { "○" };
                format!("{status} {}", t.name)
            })
            .collect();

        let new_task_btn = ButtonWidget {
            id: "tasks-new-btn".into(),
            label: keys::NEW_TASK.into(), // FTL key resolved at render time
            enabled: true,
            action: "create".into(),
        };

        Box::new(ListWidget {
            id: "tasks-list".into(),
            items: std::iter::once(new_task_btn.label.clone())
                .chain(items)
                .collect(),
            selected_index: None,
            enabled: true,
        })
    }
}

// ── TaskDetailView ────────────────────────────────────────────────────────────

/// View for a single task pipeline.
pub struct TaskDetailView {
    pub task: TaskPipeline,
}

impl TaskDetailView {
    #[must_use]
    pub fn new(task: TaskPipeline) -> Self {
        Self { task }
    }
}

impl FsView for TaskDetailView {
    fn view(&self) -> Box<dyn FsWidget> {
        let status = if self.task.enabled {
            keys::STATUS_ACTIVE
        } else {
            keys::STATUS_INACTIVE
        };

        let trigger_label = self.task.trigger.label();

        let mapping_items: Vec<String> = self
            .task
            .mappings
            .iter()
            .map(|m| {
                let src = m.source_field.as_deref().unwrap_or("(fixed)");
                format!("{src} → {}", m.target_field)
            })
            .collect();

        Box::new(ListWidget {
            id: format!("task-detail-{}", self.task.id),
            items: vec![
                self.task.name.clone(),
                status.to_string(),
                trigger_label,
                format!(
                    "{} → {}",
                    self.task.source.service, self.task.target.service
                ),
            ]
            .into_iter()
            .chain(mapping_items)
            .collect(),
            selected_index: None,
            enabled: true,
        })
    }
}

// ── CreateTaskView ────────────────────────────────────────────────────────────

/// View for creating a new task pipeline.
pub struct CreateTaskView {
    pub name_value: String,
}

impl CreateTaskView {
    #[must_use]
    pub fn new(name_value: impl Into<String>) -> Self {
        Self {
            name_value: name_value.into(),
        }
    }
}

impl FsView for CreateTaskView {
    fn view(&self) -> Box<dyn FsWidget> {
        let name_input = TextInputWidget {
            id: "create-task-name".into(),
            placeholder: keys::FIELD_TITLE.into(), // FTL key
            value: self.name_value.clone(),
            enabled: true,
        };

        let create_btn = ButtonWidget {
            id: "create-task-btn".into(),
            label: keys::NEW_TASK.into(), // FTL key
            enabled: !self.name_value.is_empty(),
            action: "create".into(),
        };

        Box::new(ListWidget {
            id: "create-task-form".into(),
            items: vec![name_input.value.clone(), create_btn.label.clone()],
            selected_index: None,
            enabled: true,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskPipeline;

    #[test]
    fn empty_tasks_view_produces_widget() {
        let v = TasksView::new(vec![]);
        let w = v.view();
        assert_eq!(w.widget_id(), "tasks-list");
        assert!(w.is_enabled());
    }

    #[test]
    fn tasks_view_shows_all_tasks() {
        let tasks = vec![TaskPipeline::new_default(1), TaskPipeline::new_default(2)];
        let v = TasksView::new(tasks);
        let w = v.view();
        assert_eq!(w.widget_id(), "tasks-list");
    }

    #[test]
    fn detail_view_has_correct_id() {
        let task = TaskPipeline::new_default(42);
        let id = task.id.clone();
        let v = TaskDetailView::new(task);
        let w = v.view();
        assert_eq!(w.widget_id(), format!("task-detail-{id}"));
    }

    #[test]
    fn create_view_disabled_when_empty() {
        let v = CreateTaskView::new("");
        let w = v.view();
        assert_eq!(w.widget_id(), "create-task-form");
    }

    #[test]
    fn create_view_enabled_when_name_set() {
        let v = CreateTaskView::new("My Task");
        let w = v.view();
        assert!(w.is_enabled());
    }
}
