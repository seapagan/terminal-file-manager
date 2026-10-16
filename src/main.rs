use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match terminal_file_manager::run(&arguments) {
        Ok(terminal_file_manager::Invocation::Output(output)) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Ok(terminal_file_manager::Invocation::Browse(path)) => {
            match terminal_file_manager::browser::run(&path) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => failure(&error),
            }
        }
        Err(error) => failure(error),
    }
}

fn failure(error: &str) -> ExitCode {
    eprintln!("tfm: {}", terminal_file_manager::ui::clean(error));
    ExitCode::from(2)
}
