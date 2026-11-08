use std::{io::Write, process::ExitCode};

mod cli_diagnostics;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match terminal_file_manager::run(&arguments) {
        Ok(terminal_file_manager::Invocation::Output(output)) => print_output(&output),
        Ok(terminal_file_manager::Invocation::Browse(path)) => {
            match terminal_file_manager::browser::run(&path) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => failure(&error),
            }
        }
        Ok(terminal_file_manager::Invocation::BrowseOptions(options)) => {
            match terminal_file_manager::browser::run_startup(&options) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => failure(&error),
            }
        }
        Err(error) => failure(error),
    }
}

fn print_output(output: &str) -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    match writeln!(stdout, "{output}").and_then(|()| stdout.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => failure(&format!("could not write to stdout: {error}")),
    }
}

fn failure(error: &str) -> ExitCode {
    eprintln!("{}", cli_diagnostics::render(error));
    ExitCode::from(2)
}
