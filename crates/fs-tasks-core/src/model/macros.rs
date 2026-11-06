//! Macro that fixes the skeleton of every JSCalendar value object.
//!
//! Pattern: Template method (macro form) — `typed_object!` adds `@type` and the flattened `extra`
//! map (lossless round trips), so each type only lists what is its own.

macro_rules! typed_object {
    ($(#[$m:meta])* $name:ident { $($body:tt)* }) => {
        $(#[$m])*
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct $name {
            #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
            pub type_: Option<String>,
            $($body)*
            #[serde(flatten)]
            pub extra: Extra,
        }
    };
}
