use std::{env::set_current_dir, os::unix::ffi::OsStrExt, path::Path, sync::atomic::AtomicBool};

use anyhow::{anyhow, Result};
use clap::Parser;
use glob::glob;
use pystaleds::rules_checking::{respects_rules, DocstringStyle};
use rayon::prelude::*;
use walkdir::DirEntry;

#[derive(Parser)]
#[command(version, about, long_about=None)]
struct Args {
    path: String,

    #[arg(long, default_value_t = false, alias = "ah")]
    /// Will ignore hidden files.
    allow_hidden: bool,

    #[arg(long, default_value_t = false, alias = "nd")]
    /// Will consider an error for a docstring to be absent.
    forbid_no_docstring: bool,

    #[arg(long, default_value_t = false, alias = "na")]
    /// Will consider an error for an "Args" or "Parameters" section to be absent.
    forbid_no_args_in_docstring: bool,

    #[arg(long, default_value_t = false, alias = "nu")]
    /// Will consider an error for an arg in docstring to be untyped. Otherwise, only
    /// raises an error if the docstring's type and the signature's type are mismatched.
    forbid_untyped_docstrings: bool,

    #[arg(long, alias = "g")]
    /// Runs over glob matches considering root to be the path specified in the command.
    /// Disconsiders the allow_hidden flag.
    glob: Option<String>,

    #[arg(long, default_value_t, value_enum, alias = "s")]
    /// Determines the docstring style to consider for parsing.
    docstyle: DocstringStyle,
}

fn is_hidden(e: &DirEntry) -> bool {
    e.file_name()
        .to_str()
        .map_or(false, |s| s.starts_with('.') && s != ".")
}

fn main() -> Result<()> {
    let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stderr());

    tracing_subscriber::fmt()
        .without_time()
        .with_writer(non_blocking)
        .init();
    rayon::ThreadPoolBuilder::new()
        .num_threads(0)
        .stack_size(100_000_000) // TODO: Make the algorithm non-recursive and remove the stack expansion.
        .build_global()
        .unwrap();

    let args = Args::parse();

    let path = Path::new(&args.path);

    let success = if let Some(s) = &args.glob {
        set_current_dir(path)?;

        let global_success = AtomicBool::new(true);

        let paths = glob(s).unwrap();

        paths.into_iter().par_bridge().for_each(|entry| {
            let Ok(entry) = entry else {
                return;
            };

            let entry = entry.as_path();

            assess_success(entry, &args, &global_success);
        });

        global_success.into_inner()
    } else {
        let success = if path.is_dir() {
            let walk = walkdir::WalkDir::new(path);

            let global_success = AtomicBool::new(true);

            walk.into_iter()
                .filter_entry(|e| {
                    if args.allow_hidden {
                        true
                    } else {
                        !is_hidden(e)
                    }
                })
                .par_bridge()
                .for_each(|entry| {
                    let Ok(entry) = entry else {
                        return;
                    };

                    let entry = entry.path();

                    assess_success(entry, &args, &global_success)
                });

            global_success.into_inner()
        } else {
            let span = tracing::error_span!("file", file_name = path.as_os_str().to_str().unwrap());
            _ = span.enter();

            is_file_compliant(
                path,
                args.forbid_no_docstring,
                args.forbid_no_args_in_docstring,
                args.forbid_untyped_docstrings,
                args.docstyle,
            )?
        };

        success
    };

    if success {
        Ok(())
    } else {
        Err(anyhow!("found errors"))
    }
}

fn assess_success(entry: &Path, args: &Args, global_success: &AtomicBool) {
    if entry.is_file() && entry.extension() == Some(std::ffi::OsStr::from_bytes("py".as_bytes())) {
        let span = tracing::error_span!("file", file_name = entry.as_os_str().to_str().unwrap());

        _ = span.enter();

        let Ok(success) = is_file_compliant(
            entry,
            args.forbid_no_docstring,
            args.forbid_no_args_in_docstring,
            args.forbid_untyped_docstrings,
            args.docstyle,
        ) else {
            return;
        };

        if !success {
            global_success.swap(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

fn is_file_compliant(
    path: &Path,
    forbid_no_docstring: bool,
    forbid_no_args_in_docstring: bool,
    forbid_untyped_docstrings: bool,
    docstyle: DocstringStyle,
) -> Result<bool> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_python::language())?;

    let contents = std::fs::read_to_string(path)?;

    let success = respects_rules(
        &mut parser,
        &contents,
        None,
        Some(path),
        !forbid_no_docstring,
        !forbid_no_args_in_docstring,
        !forbid_untyped_docstrings,
        docstyle,
    );

    Ok(success)
}
