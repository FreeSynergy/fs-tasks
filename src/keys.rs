// keys.rs — FTL key name constants for fs-tasks.
//
// All user-visible strings are translated via fs-i18n.
// The matching .ftl files live at:
//   fs-i18n/locales/{lang}/tasks.ftl
//
// Use these constants wherever a localised string is needed.

// ── App ───────────────────────────────────────────────────────────────────────

pub const TITLE: &str = "tasks-title";
pub const HOME: &str = "tasks-home";
pub const EMPTY: &str = "tasks-empty";
pub const NEW_TASK: &str = "tasks-new-task";

// ── Status ────────────────────────────────────────────────────────────────────

pub const STATUS_ACTIVE: &str = "tasks-status-active";
pub const STATUS_INACTIVE: &str = "tasks-status-inactive";

// ── Trigger ───────────────────────────────────────────────────────────────────

pub const TRIGGER_MANUAL: &str = "tasks-trigger-manual";
pub const TRIGGER_ON_EVENT: &str = "tasks-trigger-on-event";
pub const TRIGGER_SCHEDULED: &str = "tasks-trigger-scheduled";

// Parametrised labels (resolved at display time)
pub const TRIGGER_MANUAL_LABEL: &str = "tasks-trigger-manual-label";
pub const TRIGGER_ON_EVENT_LABEL: &str = "tasks-trigger-on-event-label";
pub const TRIGGER_SCHEDULED_LABEL: &str = "tasks-trigger-scheduled-label";

// ── Transform ─────────────────────────────────────────────────────────────────

pub const TRANSFORM_DIRECT: &str = "tasks-transform-direct";
pub const TRANSFORM_TEMPLATE: &str = "tasks-transform-template";
pub const TRANSFORM_FIXED: &str = "tasks-transform-fixed";

// ── Fields ────────────────────────────────────────────────────────────────────

pub const FIELD_TITLE: &str = "tasks-field-title";
pub const FIELD_BODY: &str = "tasks-field-body";
pub const FIELD_DESCRIPTION: &str = "tasks-field-description";
pub const FIELD_REPO_NAME: &str = "tasks-field-repo-name";

// ── Field mapping ─────────────────────────────────────────────────────────────

pub const FIELD_MAPPING_TITLE: &str = "tasks-field-mapping-title";
pub const FIELD_MAPPING_FIXED: &str = "tasks-field-mapping-fixed";
pub const FIELD_MAPPING_DIRECT: &str = "tasks-field-mapping-direct-copy";

// ── Templates ─────────────────────────────────────────────────────────────────

pub const TEMPLATES_TITLE: &str = "tasks-templates-title";
pub const TEMPLATES_DESCRIPTION: &str = "tasks-templates-description";

// ── Pipeline editor ───────────────────────────────────────────────────────────

pub const PIPELINE_TITLE: &str = "tasks-pipeline-title";
