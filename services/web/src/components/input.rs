use leptos::ev;
use leptos::prelude::*;

use super::utils::cn;

/// An accessible text input component.
///
/// Supports `aria-invalid` and `aria-describedby` for error states.
/// Pair with `<Label>` using matching `id`/`for` attributes.
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn Input(
    #[prop(optional, into)] id: Option<String>,
    #[prop(optional, into)] name: Option<String>,
    #[prop(optional, into)] r#type: Option<String>,
    #[prop(optional, into)] placeholder: Option<String>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] invalid: Signal<bool>,
    #[prop(optional, into)] error_id: Option<String>,
    #[prop(optional, into)] disabled: Signal<bool>,
    #[prop(optional, into)] required: bool,
    #[prop(optional)] on_input: Option<Box<dyn Fn(ev::Event)>>,
) -> impl IntoView {
    let base = "flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50";
    let invalid_class = "border-destructive focus-visible:ring-destructive";

    let classes = move || {
        if invalid.get() {
            cn(&[base, invalid_class, &class])
        } else {
            cn(&[base, &class])
        }
    };

    let input_type = r#type.unwrap_or_else(|| "text".into());

    view! {
        <input
            id=id
            name=name
            type=input_type
            placeholder=placeholder
            class=classes
            disabled=move || disabled.get()
            required=required
            aria-invalid=move || if invalid.get() { Some("true") } else { None }
            aria-describedby=move || { if invalid.get() { error_id.clone() } else { None } }
            on:input=move |ev| {
                if let Some(ref handler) = on_input {
                    handler(ev);
                }
            }
        />
    }
}
