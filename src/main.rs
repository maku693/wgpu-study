mod app;
mod renderer;

#[tokio::main]
async fn main() {
    let app = app::App::new().await;
    app.run();
}
