use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::{Button, ButtonVariant};

/// Landing page with sign-up and login CTAs.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <Title text="Home | home.ryanseipp.com" />
        <div class="flex flex-col justify-center items-center space-y-8 min-h-[80vh]">
            <h1 class="text-4xl font-bold tracking-tight sm:text-6xl">"Welcome"</h1>
            <p class="max-w-2xl text-lg text-center text-muted-foreground">
                "Your home on the web."
            </p>
            <div class="flex gap-4">
                <a href="/sign-up">
                    <Button>"Sign Up"</Button>
                </a>
                <a href="/login">
                    <Button variant=ButtonVariant::Outline>"Sign In"</Button>
                </a>
            </div>
        </div>
    }
}
