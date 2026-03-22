use leptos::ev;
use leptos::prelude::*;

use super::utils::cn;

/// Visual variant for the button.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Destructive,
}

/// Size variant for the button.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl ButtonVariant {
    fn classes(self) -> &'static str {
        match self {
            Self::Primary => "bg-primary text-primary-foreground shadow hover:bg-primary/90",
            Self::Secondary => {
                "bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80"
            }
            Self::Outline => {
                "border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground"
            }
            Self::Ghost => "hover:bg-accent hover:text-accent-foreground",
            Self::Destructive => {
                "bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90"
            }
        }
    }
}

impl ButtonSize {
    fn classes(self) -> &'static str {
        match self {
            Self::Sm => "h-8 rounded-md px-3 text-xs",
            Self::Md => "h-9 rounded-md px-4 py-2 text-sm",
            Self::Lg => "h-10 rounded-md px-8 text-base",
        }
    }
}

/// An accessible button component with variant and size support.
///
/// Renders a `<button>` element with appropriate ARIA attributes.
/// When `disabled` is true, sets `aria-disabled` and prevents interaction.
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn Button(
    #[prop(optional, into)] variant: ButtonVariant,
    #[prop(optional, into)] size: ButtonSize,
    #[prop(optional, into)] disabled: Signal<bool>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] r#type: Option<String>,
    #[prop(optional)] on_click: Option<Box<dyn Fn(ev::MouseEvent)>>,
    children: Children,
) -> impl IntoView {
    let base = "inline-flex items-center justify-center whitespace-nowrap font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50";
    let classes = cn(&[base, variant.classes(), size.classes(), &class]);
    let button_type = r#type.unwrap_or_else(|| "button".into());

    view! {
        <button
            type=button_type
            class=classes
            disabled=move || disabled.get()
            aria-disabled=move || if disabled.get() { Some("true") } else { None }
            on:click=move |ev| {
                if !disabled.get() && let Some(ref handler) = on_click {
                    handler(ev);
                }
            }
        >
            {children()}
        </button>
    }
}
