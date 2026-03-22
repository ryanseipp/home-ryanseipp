use leptos::prelude::*;

use super::utils::cn;

/// An accessible label component that associates with a form control.
///
/// Renders a `<label>` element. Use the `for_id` prop to link it
/// to an `<Input>` or other form element by its `id`.
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn Label(
    #[prop(optional, into)] for_id: Option<String>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let base = "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70";
    let classes = cn(&[base, &class]);

    view! {
        <label r#for=for_id class=classes>
            {children()}
        </label>
    }
}
