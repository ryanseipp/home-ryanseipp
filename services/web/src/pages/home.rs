use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::{Button, ButtonVariant};

/// Landing page with sign-up and login CTAs.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <Title text="Home | home.ryanseipp.com" />
        <div class="flex min-h-[80vh] flex-col items-center justify-center space-y-8">
            <h1 class="text-4xl font-bold tracking-tight sm:text-6xl">"Welcome"</h1>
            <p class="max-w-2xl text-center text-lg text-muted-foreground">
                "Your home on the web."
            </p>
            <div class="flex gap-4">
                <a href="/sign-up">
                    <Button>"Get Started"</Button>
                </a>
                <a href="/login">
                    <Button variant=ButtonVariant::Outline>"Sign In"</Button>
                </a>
            </div>
        </div>
    }
}
