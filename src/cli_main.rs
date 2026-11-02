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
