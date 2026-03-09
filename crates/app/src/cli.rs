//! CLI argument parsing for rira.
//!
//! Supports `rira [file[:line]]` format to open a file at a specific line.

use std::path::PathBuf;

use clap::Parser;

/// rira — Rust native code editor
#[derive(Parser, Debug)]
#[command(name = "rira", version, about = "Rust native code editor")]
pub struct CliArgs {
    /// File to open, optionally with a line number (e.g. file.rs:42)
    pub file: Option<String>,
}

/// Parse a `file[:line]` argument into a path and optional line number.
///
/// If the line portion is missing or not a valid number, `None` is returned
/// for the line number.  The line number is returned as-is (1-based) and
/// the caller is responsible for converting to 0-based if needed.
#[must_use]
pub fn parse_file_arg(arg: &str) -> (PathBuf, Option<usize>) {
    // We need to handle paths that may contain colons (e.g. on Windows C:\...)
    // Strategy: split from the right on ':', check if the right part is a number.
    if let Some(pos) = arg.rfind(':') {
        let (path_part, line_part) = arg.split_at(pos);
        // line_part starts with ':', skip it
        let line_str = &line_part[1..];

        if !path_part.is_empty() {
            if let Ok(line) = line_str.parse::<usize>() {
                return (PathBuf::from(path_part), Some(line));
            }
        }
    }

    (PathBuf::from(arg), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filename_only() {
        let (path, line) = parse_file_arg("file.rs");
        assert_eq!(path, PathBuf::from("file.rs"));
        assert_eq!(line, None);
    }

    #[test]
    fn parse_filename_with_line() {
        let (path, line) = parse_file_arg("file.rs:42");
        assert_eq!(path, PathBuf::from("file.rs"));
        assert_eq!(line, Some(42));
    }

    #[test]
    fn parse_relative_path_with_line() {
        let (path, line) = parse_file_arg("path/to/file.rs:1");
        assert_eq!(path, PathBuf::from("path/to/file.rs"));
        assert_eq!(line, Some(1));
    }

    #[test]
    fn parse_line_zero() {
        let (path, line) = parse_file_arg("file.rs:0");
        assert_eq!(path, PathBuf::from("file.rs"));
        assert_eq!(line, Some(0));
    }

    #[test]
    fn parse_invalid_line_number() {
        let (path, line) = parse_file_arg("file.rs:abc");
        assert_eq!(path, PathBuf::from("file.rs:abc"));
        assert_eq!(line, None);
    }

    #[test]
    fn parse_absolute_path_with_line() {
        let (path, line) = parse_file_arg("/absolute/path.rs:10");
        assert_eq!(path, PathBuf::from("/absolute/path.rs"));
        assert_eq!(line, Some(10));
    }

    #[test]
    fn parse_file_with_spaces() {
        let (path, line) = parse_file_arg("file with spaces.rs:5");
        assert_eq!(path, PathBuf::from("file with spaces.rs"));
        assert_eq!(line, Some(5));
    }

    #[test]
    fn parse_file_with_spaces_no_line() {
        let (path, line) = parse_file_arg("file with spaces.rs");
        assert_eq!(path, PathBuf::from("file with spaces.rs"));
        assert_eq!(line, None);
    }

    #[test]
    fn parse_trailing_colon() {
        // "file.rs:" — empty line part, not a valid number
        let (path, line) = parse_file_arg("file.rs:");
        assert_eq!(path, PathBuf::from("file.rs:"));
        assert_eq!(line, None);
    }
}
