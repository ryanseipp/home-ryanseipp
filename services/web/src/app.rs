use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{Route, Router, Routes},
};

use crate::pages::{HomePage, LoginPage, NotFoundPage, SignUpPage};

/// Set the `Content-Security-Policy` response header using the nonce that
/// `leptos_axum` generated for this request.  Runs only during SSR — on the
/// client this is a no-op.
#[cfg(feature = "ssr")]
fn set_csp_header() {
    use leptos::context::use_context;
    use leptos::nonce::use_nonce;
    use leptos_axum::ResponseOptions;

    if let (Some(resp), Some(nonce)) = (use_context::<ResponseOptions>(), use_nonce()) {
        let connect_src = if cfg!(debug_assertions) {
            // Allow WebSocket to cargo-leptos reload port in dev
            "connect-src 'self' ws://localhost:3001 ws://127.0.0.1:3001"
        } else {
            "connect-src 'self'"
        };
        let csp = format!(
            "default-src 'self'; \
             script-src 'strict-dynamic' 'nonce-{nonce}' 'wasm-unsafe-eval'; \
             style-src 'self'; \
             img-src 'self'; \
             font-src 'self'; \
             {connect_src}; \
             frame-ancestors 'none'; \
             base-uri 'self'; \
             form-action 'self'"
        );
        resp.insert_header(
            axum::http::header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_str(&csp).expect("valid CSP header value"),
        );
    }
}

#[cfg(not(feature = "ssr"))]
fn set_csp_header() {}

/// HTML shell rendered on the server for SSR.
///
/// Provides the document structure, hydration scripts, and meta tag injection
/// point. `cargo-leptos` hot-reloads the stylesheet referenced by `id="leptos"`.
#[allow(clippy::needless_pass_by_value)]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body class="min-h-screen antialiased bg-background text-foreground">
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    set_csp_header();

    view! {
        <Stylesheet id="leptos" href="/pkg/web.css" />
        <Title text="home.ryanseipp.com" />

        <Router>
            <main class="px-4 mx-auto max-w-7xl sm:px-6 lg:px-8">
                <Routes fallback=|| NotFoundPage().into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=StaticSegment("login") view=LoginPage />
                    <Route path=StaticSegment("sign-up") view=SignUpPage />
                </Routes>
            </main>
        </Router>
    }
}
