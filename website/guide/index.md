# Getting Started

## Prerequisites

- Rust 1.70+
- A terminal that supports Ratatui (most modern terminals do)

## Create a new project

```bash
cargo new my-app
cd my-app
```

Add Santui and Ratatui to your `Cargo.toml`:

```toml
[dependencies]
santui-core = { git = "https://github.com/sonyak/santui" }
ratatui = "0.26"
```

## Minimal example

```rust
use santui_core::app::App;
use santui_core::event::Event;
use santui_core::widget::{Widget, WidgetContext};
use ratatui::Frame;

struct HelloWidget;

impl Widget for HelloWidget {
    fn render(&mut self, ctx: &mut WidgetContext, frame: &mut Frame) {
        frame.render_widget(
            ratatui::widgets::Paragraph::new("Hello, Santui!"),
            frame.area(),
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new()?;
    app.add_widget(Box::new(HelloWidget));
    app.run()
}
```
