use std::ffi::OsString;
use std::path::PathBuf;

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, Result};

use super::LayoutRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Generate,
    CheckCorpus,
    CheckTaffyCorpus,
    ImportTaffy,
}

pub(super) fn run_from_env() -> Result<()> {
    if let Some(result) = super::supervisor::run_from_env_if_present() {
        return result;
    }
    run_from_args(std::env::args_os().skip(1))
}

fn run_from_args(arguments: impl IntoIterator<Item = OsString>) -> Result<()> {
    let mut arguments = arguments.into_iter();
    let mut owner_root = None;
    let mut corpus_root = None;
    let mut source_root = None;
    let mut browser_path = None;
    let mut filter = None;
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
            Some("--browser-path") => set_once(
                &mut browser_path,
                next_value(&mut arguments, "--browser-path")?,
                "--browser-path",
            )?,
            Some("--filter") => set_once(
                &mut filter,
                next_value(&mut arguments, "--filter")?,
                "--filter",
            )?,
            Some(value) if value.starts_with("--") => {
                return Err(cli_error(format!("unknown flag: {value}")));
            }
            Some("check-taffy-corpus") => {
                set_once(&mut command, Command::CheckTaffyCorpus, "layout command")?
            }
            Some("check-corpus") => set_once(&mut command, Command::CheckCorpus, "layout command")?,
            Some("import-taffy") => set_once(&mut command, Command::ImportTaffy, "layout command")?,
            Some("generate") => set_once(&mut command, Command::Generate, "layout command")?,
            Some(value) => return Err(cli_error(format!("unknown layout command: {value}"))),
            None => return Err(cli_error("layout command name must be UTF-8")),
        }
    }

    let owner_root = required_path(owner_root, "--owner-root")?;
    let corpus_root = required_path(corpus_root, "--corpus-root")?;
    let command = command.ok_or_else(|| cli_error("missing layout command"))?;
    let (source_root, browser_path, filter) = match command {
        Command::Generate => {
            if source_root.is_some() {
                return Err(cli_error("generate forbids --source-root"));
            }
            let browser = required_relative(browser_path, "--browser-path")?;
            let filter = filter
                .map(|value| relative_value(value, "--filter"))
                .transpose()?;
            if filter.as_ref().is_some_and(|path| {
                super::manifest::paths_target_equal(path.as_str(), super::manifest::SIDECAR_FILE)
            }) {
                return Err(cli_error(
                    "generation filter uses the reserved Taffy sidecar path",
                ));
            }
            (None, Some(browser), filter)
        }
        Command::CheckCorpus => {
            if source_root.is_some() {
                return Err(cli_error("check-corpus forbids --source-root"));
            }
            forbid_generation_options("check-corpus", &browser_path, &filter)?;
            (None, None, None)
        }
        Command::CheckTaffyCorpus | Command::ImportTaffy => {
            let name = if command == Command::CheckTaffyCorpus {
                "check-taffy-corpus"
            } else {
                "import-taffy"
            };
            forbid_generation_options(name, &browser_path, &filter)?;
            (
                Some(required_path(source_root, "--source-root")?),
                None,
                None,
            )
        }
    };
    let location = CorpusLocation::new(owner_root, corpus_root)?;
    let request = match command {
        Command::CheckCorpus => LayoutRequest::check_corpus(location),
        Command::CheckTaffyCorpus => LayoutRequest::check_taffy_corpus(
            location,
            source_root.expect("checked Taffy command source root"),
        )?,
        Command::ImportTaffy => LayoutRequest::import_taffy(
            location,
            source_root.expect("checked Taffy command source root"),
        )?,
        Command::Generate => LayoutRequest::generate(
            location,
            browser_path.expect("checked generation browser path"),
            filter,
        )?,
    };
    super::run(request)
}

fn forbid_generation_options(
    command: &str,
    browser_path: &Option<OsString>,
    filter: &Option<OsString>,
) -> Result<()> {
    if browser_path.is_some() {
        return Err(cli_error(format!("{command} forbids --browser-path")));
    }
    if filter.is_some() {
        return Err(cli_error(format!("{command} forbids --filter")));
    }
    Ok(())
}

fn required_relative(value: Option<OsString>, flag: &str) -> Result<RelativePath> {
    relative_value(
        value.ok_or_else(|| cli_error(format!("missing {flag}")))?,
        flag,
    )
}

fn relative_value(value: OsString, flag: &str) -> Result<RelativePath> {
    let value = value
        .into_string()
        .map_err(|_| cli_error(format!("{flag} must be UTF-8")))?;
    RelativePath::new(&value).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Cli,
            "parse layout command line",
            format!("invalid {flag}: {value}"),
            source,
        )
    })
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
