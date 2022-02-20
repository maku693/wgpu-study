mod app;
mod renderer;

#[tokio::main]
async fn main() {
    app::App::new().await.run();
}
