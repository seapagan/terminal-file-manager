//! Conservative styling for stderr diagnostics; capability policy belongs to colored_text.
use colored_text::{Colorize, RenderTarget};
use terminal_file_manager::ui::clean;

pub(super) fn render(error: &str) -> String {
    let mut lines = error.split('\n').map(clean);
    let mut output = format!(
        "{} {}",
        "tfm:".red().bold().render(RenderTarget::Stderr),
        header(&lines.next().unwrap())
    );
    let mut explanation = false;
    for line in lines {
        output.push('\n');
        if let Some((margin, source)) = gutter(&line) {
            output.push_str(&margin.dim().render(RenderTarget::Stderr));
            let pointer = margin.trim() == "|"
                && !source.trim().is_empty()
                && source.trim().bytes().all(|byte| byte == b'^');
            if pointer {
                output.push_str(&source.bright_red().render(RenderTarget::Stderr));
                explanation = true;
            } else {
                output.push_str(source);
            }
        } else if explanation {
            output.push_str(&summary(&line));
            explanation = false;
        } else {
            output.push_str(&line);
        }
    }
    output
}

fn header(line: &str) -> String {
    // Only recognise an absolute path when the producer's separator is unambiguous.
    if let Some((path, description)) = line.split_once(": ")
        && std::path::Path::new(path).is_absolute()
        && !description.contains(": ")
    {
        return format!(
            "{}: {}",
            path.cyan().render(RenderTarget::Stderr),
            description.red().bold().render(RenderTarget::Stderr)
        );
    }
    line.red().bold().render(RenderTarget::Stderr)
}

fn gutter(line: &str) -> Option<(&str, &str)> {
    let (number, _) = line.split_once('|')?;
    if !number.trim().bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(line.split_at(number.len() + 1))
}

fn summary(line: &str) -> String {
    if let Some((phrase, rest)) = line.split_once(", ")
        && !phrase.is_empty()
        && !rest.is_empty()
    {
        return format!("{}, {rest}", phrase.yellow().render(RenderTarget::Stderr));
    }
    line.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutters_require_an_empty_or_numeric_margin_and_preserve_source() {
        assert_eq!(gutter(" 12 | x | y"), Some((" 12 |", " x | y")));
        assert_eq!(gutter("    |  ^^^"), Some(("    |", "  ^^^")));
        for line in [
            "ordinary prose",
            "text | source",
            "1 2 | source",
            "-1 | source",
        ] {
            assert_eq!(gutter(line), None);
        }
    }

    #[test]
    fn generic_diagnostics_preserve_whitespace_and_escape_untrusted_controls() {
        let input = "parser failed\n\n 12 | λ\\n\u{1b}[31m\u{202e}\r\n    | ^^^\ninvalid value, expected a number\n\nprose | unchanged\n";
        let plain = regex::Regex::new("\u{1b}\\[[0-9;]*m")
            .unwrap()
            .replace_all(&render(input), "")
            .into_owned();
        assert_eq!(
            plain,
            "tfm: parser failed\n\n 12 | λ\\n\\u{1b}[31m\\u{202e}\\r\n    | ^^^\ninvalid value, expected a number\n\nprose | unchanged\n"
        );
    }
}
