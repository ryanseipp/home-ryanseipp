use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::{Button, Card, CardContent, CardFooter, CardHeader, Input, Label};

/// Login page with email/password form.
#[component]
pub fn LoginPage() -> impl IntoView {
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());

    view! {
        <Title text="Sign In | home.ryanseipp.com" />
        <div class="flex min-h-[80vh] items-center justify-center">
            <Card class="w-full max-w-md">
                <CardHeader>
                    <h1 class="text-2xl font-semibold tracking-tight">"Sign In"</h1>
                    <p class="text-sm text-muted-foreground">
                        "Enter your credentials to access your account."
                    </p>
                </CardHeader>
                <CardContent>
                    <form class="space-y-4" method="post" action="/api/v1/login">
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
                                placeholder="Enter your password"
                                required=true
                                on_input=Box::new(move |ev| {
                                    password.set(event_target_value(&ev));
                                })
                            />
                        </div>
                        <Button r#type="submit".to_owned() class="w-full">
                            "Sign In"
                        </Button>
                    </form>
                </CardContent>
                <CardFooter>
                    <p class="text-sm text-muted-foreground">
                        "Don't have an account? "
                        <a href="/sign-up" class="underline underline-offset-4 hover:text-primary">
                            "Sign up"
                        </a>
                    </p>
                </CardFooter>
            </Card>
        </div>
    }
}
