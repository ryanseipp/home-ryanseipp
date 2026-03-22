use leptos::prelude::*;

use super::utils::cn;

/// A card container with optional header, content, and footer sections.
///
/// Provides visual grouping with rounded borders and shadow.
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn Card(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let base = "rounded-xl border bg-card text-card-foreground shadow";
    let classes = cn(&[base, &class]);

    view! { <div class=classes>{children()}</div> }
}

/// Header section of a `<Card>`.
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn CardHeader(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let base = "flex flex-col space-y-1.5 p-6";
    let classes = cn(&[base, &class]);

    view! { <div class=classes>{children()}</div> }
}

/// Content section of a `<Card>`.
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn CardContent(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let base = "p-6 pt-0";
    let classes = cn(&[base, &class]);

    view! { <div class=classes>{children()}</div> }
}

/// Footer section of a `<Card>`.
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn CardFooter(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let base = "flex items-center p-6 pt-0";
    let classes = cn(&[base, &class]);

    view! { <div class=classes>{children()}</div> }
}
