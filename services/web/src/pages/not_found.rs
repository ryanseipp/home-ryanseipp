use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::{Button, ButtonVariant};

/// 404 not found page.
#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <Title text="Not Found | home.ryanseipp.com" />
        <div class="flex min-h-[80vh] flex-col items-center justify-center space-y-4">
            <h1 class="text-6xl font-bold">"404"</h1>
            <p class="text-lg text-muted-foreground">"Page not found."</p>
            <a href="/">
                <Button variant=ButtonVariant::Outline>"Go Home"</Button>
            </a>
        </div>
    }
}
