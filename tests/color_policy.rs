//! Exercise the actual colour policy in isolated child environments.
use colored_text::{ColorizeConfig, RenderTarget};
use std::process::Command;
use terminal_file_manager::color::{Role, Theme};

#[test]
fn environment_policy_preserves_no_color_and_force_precedence() {
    if let Ok(expected) = std::env::var("TFM_EXPECT_LEVEL") {
        let level = ColorizeConfig::color_level(RenderTarget::Stdout);
        assert_eq!(format!("{level:?}"), expected);
        let style = Theme::new(level).style(Role::Directory);
        assert_eq!(style.fg.is_none(), expected == "NoColor");
        return;
    }
    for (values, expected) in [
        (vec![("NO_COLOR", "1"), ("FORCE_COLOR", "3")], "NoColor"),
        (vec![("NO_COLOR", ""), ("CLICOLOR_FORCE", "1")], "NoColor"),
        (vec![("FORCE_COLOR", "0")], "NoColor"),
        (vec![("FORCE_COLOR", "1")], "Ansi16"),
        (vec![("FORCE_COLOR", "2")], "Ansi256"),
        (vec![("FORCE_COLOR", "3"), ("CLICOLOR", "0")], "TrueColor"),
        (vec![("CLICOLOR", "0")], "NoColor"),
        (vec![("TERM", "dumb")], "NoColor"),
    ] {
        let mut child = Command::new(std::env::current_exe().unwrap());
        for key in [
            "NO_COLOR",
            "FORCE_COLOR",
            "CLICOLOR_FORCE",
            "CLICOLOR",
            "TERM",
        ] {
            child.env_remove(key);
        }
        let result = child
            .args([
                "--exact",
                "environment_policy_preserves_no_color_and_force_precedence",
            ])
            .envs(values)
            .env("TFM_EXPECT_LEVEL", expected)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
}
