use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::{Button, Card, CardContent, CardFooter, CardHeader, Input, Label};

/// Sign-up page with registration form.
#[component]
pub fn SignUpPage() -> impl IntoView {
    let username = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());

    view! {
        <Title text="Sign Up | home.ryanseipp.com" />
        <div class="flex min-h-[80vh] items-center justify-center">
            <Card class="w-full max-w-md">
                <CardHeader>
                    <h1 class="text-2xl font-semibold tracking-tight">"Create Account"</h1>
                    <p class="text-sm text-muted-foreground">
                        "Enter your details to create a new account."
                    </p>
                </CardHeader>
                <CardContent>
                    <form class="space-y-4" method="post" action="/api/v1/sign-up">
                        <div class="space-y-2">
                            <Label for_id="username">"Username"</Label>
                            <Input
                                id="username"
                                name="username"
                                placeholder="Choose a username"
                                required=true
                                on_input=Box::new(move |ev| {
                                    username.set(event_target_value(&ev));
                                })
                            />
                        </div>
                        <div class="space-y-2">
                            <Label for_id="email">"Email"</Label>
                            <Input
                                id="email"
                                name="email"
                                r#type="email"
                                placeholder="you@example.com"
                                required=true
                                on_input=Box::new(move |ev| {
                                    email.set(event_target_value(&ev));
                                })
                            />
                        </div>
                        <div class="space-y-2">
                            <Label for_id="password">"Password"</Label>
                            <Input
                                id="password"
                                name="password"
                                r#type="password"
                                placeholder="Choose a password"
                                required=true
                                on_input=Box::new(move |ev| {
                                    password.set(event_target_value(&ev));
                                })
                            />
                        </div>
                        <Button r#type="submit".to_owned() class="w-full">
                            "Create Account"
                        </Button>
                    </form>
                </CardContent>
                <CardFooter>
                    <p class="text-sm text-muted-foreground">
                        "Already have an account? "
                        <a href="/login" class="underline underline-offset-4 hover:text-primary">
                            "Sign in"
                        </a>
                    </p>
                </CardFooter>
            </Card>
        </div>
    }
}
