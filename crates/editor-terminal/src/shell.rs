//! Native shell integration, including current-directory tracking.
use crate::Profile;
use base64::Engine;

/// Preserve a user's PowerShell prompt and emit private, nonprinting cwd metadata before it.
pub fn arguments(profile: &Profile) -> Vec<String> {
    let mut args = profile.args.clone();
    let tool = profile
        .program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        tool.as_str(),
        "powershell.exe" | "powershell" | "pwsh.exe" | "pwsh"
    ) && !args.iter().any(|a| {
        matches!(
            a.to_ascii_lowercase().as_str(),
            "-command" | "-c" | "-file" | "-f" | "-encodedcommand"
        )
    }) {
        args.extend(["-NoExit".into(),"-Command".into(),r#"$global:__MeEditorPrompt=$function:prompt; function global:prompt { $p=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes((Get-Location).ProviderPath)); [Console]::Write(([char]27).ToString()+']633;P;Cwd64='+$p+[char]7); & $global:__MeEditorPrompt }"#.into()]);
    }
    args
}

/// Recognize only the default empty PowerShell prompt for the legacy spacing migration.
pub fn default_prompt(profile: &Profile, cwd: &str) -> Option<String> {
    let tool = profile
        .program
        .rsplit(['/', '\\'])
        .next()?
        .to_ascii_lowercase();
    if !matches!(
        tool.as_str(),
        "powershell.exe" | "powershell" | "pwsh.exe" | "pwsh"
    ) {
        return None;
    }
    // The PTY removes device prefixes before launching the shell; match the same displayed path.
    let cwd = if let Some(path) = cwd.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{path}")
    } else {
        cwd.strip_prefix(r"\\?\").unwrap_or(cwd).to_owned()
    };
    Some(format!("PS {}>", cwd.replace('/', "\\")))
}

/// Quote user-selected `paths` as literal arguments for this profile's shell, without executing them.
/// Unknown shells use POSIX single quotes. Unsafe cmd.exe expansion syntax is rejected explicitly.
pub fn quote_paths(profile: &Profile, paths: &[std::path::PathBuf]) -> Option<String> {
    let tool = profile
        .program
        .rsplit(['/', '\\'])
        .next()?
        .to_ascii_lowercase();
    paths
        .iter()
        .map(|path| {
            let path = path.to_str()?;
            if path.chars().any(char::is_control) {
                return None;
            }
            Some(
                if matches!(
                    tool.as_str(),
                    "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
                ) {
                    format!("'{}'", path.replace('\'', "''"))
                } else if matches!(tool.as_str(), "cmd" | "cmd.exe") {
                    // Percent and delayed expansion still execute inside cmd's double quotes.
                    if path.contains(['%', '!', '"']) {
                        return None;
                    }
                    format!("\"{path}\"")
                } else {
                    format!("'{}'", path.replace('\'', "'\"'\"'"))
                },
            )
        })
        .collect::<Option<Vec<_>>>()
        .map(|paths| paths.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shell-specific quoting preserves spaces, quotes and Unicode without interpolating path syntax.
    #[test]
    fn selected_paths_are_literal_shell_arguments() {
        let profile = |program: &str| Profile {
            name: program.into(),
            program: program.into(),
            args: vec![],
        };
        let paths = [std::path::PathBuf::from("C:/含 空格/a'b.txt")];
        assert_eq!(
            quote_paths(&profile("pwsh.exe"), &paths).as_deref(),
            Some("'C:/含 空格/a''b.txt'")
        );
        assert_eq!(
            quote_paths(&profile("bash"), &paths).as_deref(),
            Some("'C:/含 空格/a'\"'\"'b.txt'")
        );
        assert_eq!(
            quote_paths(&profile("cmd.exe"), &["C:/%PATH%/x".into()]),
            None
        );
        assert_eq!(
            quote_paths(&profile("cmd.exe"), &["C:/a b.txt".into()]).as_deref(),
            Some("\"C:/a b.txt\"")
        );
    }
}
#[derive(Default)]
pub(super) struct Metadata {
    pub cwd: Option<String>,
}
impl vte::Perform for Metadata {
    /// Accept only bounded cwd metadata; VT screen behavior stays with the upstream parser.
    fn osc_dispatch(&mut self, params: &[&[u8]], _: bool) {
        if params.len() == 3 && params[0] == b"633" && params[1] == b"P" {
            if let Some(encoded) = params[2].strip_prefix(b"Cwd64=") {
                if encoded.len() <= 16384 {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) {
                        if let Ok(path) = String::from_utf8(bytes) {
                            if !path.is_empty() && !path.contains('\0') {
                                self.cwd = Some(path);
                            }
                        }
                    }
                }
            }
        }
    }
}
