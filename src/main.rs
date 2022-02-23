use anyhow::{Context, Result};

mod app;
mod renderer;

fn main() -> Result<()> {
    env_logger::init();
    let app = app::App::new().context("failed to iniiatlize app")?;
    app.run();
    Ok(())
}
