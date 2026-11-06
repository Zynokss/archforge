#[allow(dead_code)]
mod system;
mod tui;
struct Operation;
impl Operation {
    fn new() -> Self {
        Self
    }
}
mod cli;
mod package_variants;
mod wiki_data;
fn main() -> std::process::ExitCode {
    if cli::run() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

#[cfg(test)]
mod terminal_regression {
    #[test]
    #[ignore = "launched inside an isolated PTY by scripts/test-cli-lifecycle.py"]
    fn session() {
        let marker =
            std::env::var_os("ARCHFORGE_TEST_READY").expect("use scripts/test-cli-lifecycle.py");
        if std::env::var_os("ARCHFORGE_TEST_EVENT_READER").is_some() {
            // Keep hangup from terminating the probe before it exercises the reader.
            let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            signal_hook::flag::register(signal_hook::consts::SIGHUP, done).unwrap();
            crossterm::terminal::enable_raw_mode().unwrap();
            std::fs::write(&marker, "ready").unwrap();
            loop {
                match crossterm::event::poll(std::time::Duration::from_secs(10)) {
                    Err(error) => {
                        std::fs::write(&marker, format!("{error:?}")).unwrap();
                        break;
                    }
                    Ok(true) => {
                        let _ = crossterm::event::read();
                    }
                    Ok(false) => (),
                }
            }
        } else {
            std::fs::write(marker, "ready").unwrap();
            // A terminal hangup can make drawing/cleanup fail; it must still exit.
            let _ = crate::tui::run();
        }
        // The test harness would print to the disconnected PTY and panic on EIO.
        std::process::exit(0);
    }
}
