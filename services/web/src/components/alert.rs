use leptos::prelude::*;

use super::utils::cn;

/// Visual variant for alerts.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum AlertVariant {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl AlertVariant {
    fn classes(self) -> &'static str {
        match self {
            Self::Info => "border-border text-foreground",
            Self::Success => "border-green-500/50 text-green-700 dark:text-green-400",
            Self::Warning => "border-yellow-500/50 text-yellow-700 dark:text-yellow-400",
            Self::Error => "border-destructive/50 text-destructive dark:text-red-400",
        }
    }

    fn role(self) -> &'static str {
        match self {
            Self::Info | Self::Success => "status",
            Self::Warning | Self::Error => "alert",
        }
    }
}

/// An accessible alert component.
///
/// Uses `role="alert"` for warning/error variants (assertive) and
/// `role="status"` for info/success (polite) to ensure screen readers
/// announce the message appropriately.
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn Alert(
    #[prop(optional, into)] variant: AlertVariant,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let base = "relative w-full rounded-lg border px-4 py-3 text-sm [&>svg+div]:translate-y-[-3px] [&>svg]:absolute [&>svg]:left-4 [&>svg]:top-4 [&>svg~*]:pl-7";
    let classes = cn(&[base, variant.classes(), &class]);

    view! {
        <div class=classes role=variant.role()>
            {children()}
        </div>
    }
}
