use anyhow::{Context, Result};

mod app;
mod renderer;

fn main() -> Result<()> {
    let app = app::App::new().context("failed to iniiatlize app")?;
    app.run();
    Ok(())
}
