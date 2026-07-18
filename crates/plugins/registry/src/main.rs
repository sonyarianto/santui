mod app;

use std::io::{self, BufRead};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .init();

    let mut app = app::App::new();
    let mut stdin = io::stdin().lock();
    let mut line = String::new();

    loop {
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: santui_ipc::protocol::HostMsg = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("[registry-plugin] Failed to parse HostMsg: {e}");
                        continue;
                    }
                };
                let response = app.handle(msg);
                let mut out = io::stdout().lock();
                let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &response);
            }
        }
    }
}
