use std::{os::unix::ffi::OsStrExt, path::Path, sync::atomic::AtomicBool};

use anyhow::{anyhow, Result};
use clap::Parser;
use pydcstrngs::parse_file_contents;
use rayon::prelude::*;
use walkdir::DirEntry;

#[derive(Parser)]
#[command(version, about, long_about=None)]
struct Args {
    path: String,

    #[arg(long, default_value_t = true, alias = "nh")]
    /// Will ignore hidden files.
    ignore_hidden: bool,

    #[arg(long, default_value_t = false, alias = "nd")]
    /// Will consider an error for a docstring to be absent.
    forbid_no_docstring: bool,

    #[arg(long, default_value_t = false, alias = "na")]
    /// Will consider an error for an "Args" section to be absent.
    forbid_no_args_in_docstring: bool,

    #[arg(long, default_value_t = false, alias = "nu")]
    /// Will consider an error for an arg in docstring to be untyped.
    forbid_untyped_docstrings: bool,
}

fn is_hidden(e: &DirEntry) -> bool {
    e.file_name()
        .to_str()
        .map_or(false, |s| s.starts_with('.') && s != ".")
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    let args = Args::parse();

    let path = Path::new(&args.path);

    let success = if path.is_dir() {
        let walk = walkdir::WalkDir::new(path);

        let global_success = AtomicBool::new(true);

        walk.into_iter()
            .filter_entry(|e| {
                if args.ignore_hidden {
                    !is_hidden(e)
                } else {
                    true
                }
            })
            .par_bridge()
            .for_each(|entry| {
                let Ok(entry) = entry else {
                    return;
                };

                if entry.path().is_file()
                    && entry.path().extension()
                        == Some(std::ffi::OsStr::from_bytes("py".as_bytes()))
                {
                    let span = tracing::error_span!(
                        "file",
                        file_name = entry.path().as_os_str().to_str().unwrap()
                    );

                    _ = span.enter();

                    let Ok(success) = parse_file(
                        entry.path(),
                        args.forbid_no_docstring,
                        args.forbid_no_args_in_docstring,
                        args.forbid_untyped_docstrings,
                    ) else {
                        return;
                    };

                    if !success {
                        global_success.swap(false, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            });

        global_success.into_inner()
    } else {
        let span = tracing::error_span!("file", file_name = path.as_os_str().to_str().unwrap());
        _ = span.enter();

        parse_file(
            path,
            args.forbid_no_docstring,
            args.forbid_no_args_in_docstring,
            args.forbid_untyped_docstrings,
        )?
    };

    if success {
        Ok(())
    } else {
        Err(anyhow!("found errors"))
    }
}

fn parse_file(
    path: &Path,
    forbid_no_docstring: bool,
    forbid_no_args_in_docstring: bool,
    forbid_untyped_docstrings: bool,
) -> Result<bool> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_python::language())?;

    let contents = std::fs::read_to_string(path)?;

    let success = parse_file_contents(
        &mut parser,
        &contents,
        None,
        Some(path),
        !forbid_no_docstring,
        !forbid_no_args_in_docstring,
        !forbid_untyped_docstrings,
    );

    Ok(success)
}
