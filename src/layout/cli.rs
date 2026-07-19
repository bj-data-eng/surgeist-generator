use std::ffi::OsString;
use std::path::PathBuf;

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result};

use super::LayoutRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    CheckTaffyCorpus,
    ImportTaffy,
}

pub(super) fn run_from_env() -> Result<()> {
    run_from_args(std::env::args_os().skip(1))
}

fn run_from_args(arguments: impl IntoIterator<Item = OsString>) -> Result<()> {
    let mut arguments = arguments.into_iter();
    let mut owner_root = None;
    let mut corpus_root = None;
    let mut source_root = None;
    let mut command = None;

    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--owner-root") => set_once(
                &mut owner_root,
                next_value(&mut arguments, "--owner-root")?,
                "--owner-root",
            )?,
            Some("--corpus-root") => set_once(
                &mut corpus_root,
                next_value(&mut arguments, "--corpus-root")?,
                "--corpus-root",
            )?,
            Some("--source-root") => set_once(
                &mut source_root,
                next_value(&mut arguments, "--source-root")?,
                "--source-root",
            )?,
            Some(value) if value.starts_with("--") => {
                return Err(cli_error(format!("unknown flag: {value}")));
            }
            Some("check-taffy-corpus") => {
                set_once(&mut command, Command::CheckTaffyCorpus, "layout command")?
            }
            Some("import-taffy") => set_once(&mut command, Command::ImportTaffy, "layout command")?,
            Some(value) => return Err(cli_error(format!("unknown layout command: {value}"))),
            None => return Err(cli_error("layout command name must be UTF-8")),
        }
    }

    let owner_root = required_path(owner_root, "--owner-root")?;
    let corpus_root = required_path(corpus_root, "--corpus-root")?;
    let command = command.ok_or_else(|| cli_error("missing layout command"))?;
    let source_root = required_path(source_root, "--source-root")?;
    let location = CorpusLocation::new(owner_root, corpus_root)?;
    let request = match command {
        Command::CheckTaffyCorpus => LayoutRequest::check_taffy_corpus(location, source_root)?,
        Command::ImportTaffy => LayoutRequest::import_taffy(location, source_root)?,
    };
    super::run(request)
}

fn next_value(arguments: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<OsString> {
    let value = arguments
        .next()
        .ok_or_else(|| cli_error(format!("missing value for {flag}")))?;
    if value.to_str().is_some_and(|value| value.starts_with("--")) {
        return Err(cli_error(format!("missing value for {flag}")));
    }
    Ok(value)
}

fn set_once<T>(slot: &mut Option<T>, value: T, label: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(cli_error(format!("duplicate {label}")));
    }
    Ok(())
}

fn required_path(value: Option<OsString>, flag: &str) -> Result<PathBuf> {
    let value = value.ok_or_else(|| cli_error(format!("missing {flag}")))?;
    if value.is_empty() {
        return Err(cli_error(format!("{flag} must not be empty")));
    }
    Ok(PathBuf::from(value))
}

fn cli_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::Cli, "parse layout command line", detail)
}
