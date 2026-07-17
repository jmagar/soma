//! Static shell-completion script generation.
//!
//! These generators emit a flat, name-only completion list for a binary's
//! top-level subcommands. They intentionally do not attempt flag-aware or
//! nested completion — that requires a real argument-parser AST (e.g. a
//! clap `Command` tree) which this crate does not assume the consumer has.
//! A CLI with richer completion needs should generate its own script from
//! its parser and treat these as a baseline for simple dispatchers.

/// Generate a bash completion script that completes `program`'s first
/// argument against `subcommands`.
pub fn bash_completion_script(program: &str, subcommands: &[&str]) -> String {
    let words = subcommands.join(" ");
    format!(
        "_{program}_completions() {{\n    local cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n    if [ \"$COMP_CWORD\" -eq 1 ]; then\n        COMPREPLY=( $(compgen -W \"{words}\" -- \"$cur\") )\n    fi\n}}\ncomplete -F _{program}_completions {program}\n"
    )
}

/// Generate a zsh completion script that completes `program`'s first
/// argument against `subcommands`.
pub fn zsh_completion_script(program: &str, subcommands: &[&str]) -> String {
    let words = subcommands.join(" ");
    format!(
        "#compdef {program}\n_{program}() {{\n    local -a subcommands\n    subcommands=({words})\n    _describe 'command' subcommands\n}}\n_{program}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_script_lists_subcommands() {
        let script = bash_completion_script("soma", &["status", "help"]);
        assert!(script.contains("status help"));
        assert!(script.contains("complete -F _soma_completions soma"));
    }

    #[test]
    fn zsh_script_lists_subcommands() {
        let script = zsh_completion_script("soma", &["status", "help"]);
        assert!(script.contains("#compdef soma"));
        assert!(script.contains("subcommands=(status help)"));
    }
}
