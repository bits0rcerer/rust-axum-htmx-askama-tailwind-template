use askama_axum::Template;
use axum::{
    extract::{MatchedPath, Path, Query, Request},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::{collections::HashMap, net::Ipv4Addr, str::FromStr};
use tower_http::trace::TraceLayer;
use tracing::Level;
use tracing_subscriber::{filter::Directive, EnvFilter};

async fn style() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css")],
        include_str!(concat!(env!("OUT_DIR"), "/style.css")),
    )
}

async fn htmx() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../node_modules/htmx.org/dist/htmx.min.js"),
    )
}

async fn static_files(Path(path): Path<String>) -> Response {
    match StaticFiles::get(&path) {
        Some(f) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, f.metadata.mimetype())],
            f.data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/static"]
struct StaticFiles;

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
    name: &'a str,
}

async fn hello(Query(query): Query<HashMap<String, String>>) -> impl IntoResponse {
    let hello_tmpl = HelloTemplate {
        name: query.get("name").map_or("World", |s| s),
    };
    hello_tmpl.into_response()
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    setup_logging()?;

    let router = Router::new()
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);

                tracing::info_span!(
                    "http_request",
                    method = ?request.method(),
                    matched_path,
                    some_other_field = tracing::field::Empty,
                )
            }),
        )
        .route("/style.css", get(style))
        .route("/htmx.min.js", get(htmx))
        .route("/static/*file", get(static_files))
        .route("/", get(hello));

    let port = u16::from_str(option_env!("PORT").unwrap_or("8080"))?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await?;
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await?;

    Ok(())
}

fn setup_logging() -> eyre::Result<()> {
    if cfg!(debug_assertions) {
        let filter = EnvFilter::builder()
            .with_default_directive(Level::DEBUG.into())
            .from_env_lossy()
            .add_directive(Directive::from_str("hyper=info")?);

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .pretty()
            .with_file(true)
            .with_line_number(true)
            .with_thread_names(true)
            .without_time()
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
    } else {
        let filter = EnvFilter::builder()
            .with_default_directive(Level::INFO.into())
            .from_env_lossy();

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .compact()
            .with_thread_names(true)
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(())
}
