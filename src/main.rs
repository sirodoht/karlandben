use askama::Template;
use axum::{response::Html, routing::get, Router};

#[derive(Template)]
#[template(path = "index.html")]
#[allow(dead_code)]
struct IndexTemplate {
    message: &'static str,
}

async fn index() -> Html<String> {
    let tmpl = IndexTemplate {
        message: "hello",
    };
    Html(tmpl.render().unwrap())
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(index));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
