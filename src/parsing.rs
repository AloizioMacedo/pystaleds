/// Parses a google docstring into a Vec with the names of the args and their types.
///
/// # Examples
///
/// ```rust
/// use pystaleds::parsing::parse_google_docstring;
///
/// let parsed_docstring = parse_google_docstring(
///            r#""""This is my docstring!!!.
///
///    Args:
///        x: This is my first parameter.
///        y: And this is my second.
///    """#,
///            false,
///        )
///        .unwrap();
///
/// assert_eq!(parsed_docstring, vec![("x", None), ("y", None)]);
///
/// let parsed_docstring = parse_google_docstring(
///            r#""""This is my docstring!!!.
///
///    Args:
///        x (int): This is my first parameter.
///        y (float): And this is my second.
///    """#,
///            false,
///        )
///        .unwrap();
///
/// assert_eq!(parsed_docstring, vec![("x", Some("int")), ("y", Some("float"))]);
///
/// let not_a_docstring = parse_google_docstring("This is not a docstring!", false);
///
/// assert!(not_a_docstring.is_none());
///
pub fn parse_google_docstring(
    text: &str,
    break_on_empty_line: bool,
) -> Option<Vec<(&str, Option<&str>)>> {
    let (_, mut args) = text.split_once("Args:\n")?;

    if let Some(c) = args.find("Returns:\n") {
        args = &args[..c];
    };

    if break_on_empty_line {
        if let Some(c) = args.find("\n\n") {
            args = &args[..c];
        }
    };

    let first_line = args.lines().next()?;

    let indentation = first_line.chars().take_while(|c| c.is_whitespace()).count();

    let mut params = Vec::new();

    for line in args.lines() {
        if line.chars().take(indentation).all(|c| c.is_whitespace())
            && line.chars().nth(indentation).map(|c| !c.is_whitespace()) == Some(true)
        {
            let Some((arg, _)) = line.split_once(':') else {
                continue;
            };

            let arg = arg.trim();

            let Some((name, typ)) = arg.split_once(' ') else {
                params.push((arg, None));
                continue;
            };

            let typ = typ.trim_start_matches('(').trim_end_matches(')');

            params.push((name, Some(typ)));
        }
    }

    Some(params)
}

/// Parses a numpy docstring into a Vec with the names of the args and their types.
///
/// # Examples
///
/// ```rust
/// use pystaleds::parsing::parse_numpy_docstring;
///
/// let parsed_docstring = parse_numpy_docstring(
///            r#""""This is my docstring!!!.
///
///    Parameters
///    ----------
///    x
///        This is my first parameter.
///    y
///        And this is my second.
///    """#,
///            false,
///        )
///        .unwrap();
///
/// assert_eq!(parsed_docstring, vec![("x", None), ("y", None)]);
///
/// let parsed_docstring = parse_numpy_docstring(
///            r#""""This is my docstring!!!.
///
///    Parameters
///    ----------
///    x: int
///        This is my first parameter.
///    y: float
///        And this is my second.
///    """#,
///            false,
///        )
///        .unwrap();
///
/// assert_eq!(parsed_docstring, vec![("x", Some("int")), ("y", Some("float"))]);
///
/// let not_a_docstring = parse_numpy_docstring("This is not a docstring!", false);
///
/// assert!(not_a_docstring.is_none());
///
pub fn parse_numpy_docstring(
    text: &str,
    break_on_empty_line: bool,
) -> Option<Vec<(&str, Option<&str>)>> {
    let (_, mut args) = text.split_once("Parameters\n")?;

    if let Some(c) = args.find("Returns\n") {
        args = &args[..c];
    };

    if break_on_empty_line {
        if let Some(c) = args.find("\n\n") {
            args = &args[..c];
        }
    };

    let first_line = args.lines().nth(1)?;

    let indentation = first_line.chars().take_while(|c| c.is_whitespace()).count();

    let mut params = Vec::new();

    for line in args.lines().skip(1) {
        if line.chars().take(indentation).all(|c| c.is_whitespace())
            && line.chars().nth(indentation).map(|c| !c.is_whitespace()) == Some(true)
            && !line.trim().trim_end_matches(&['\'', '\"']).is_empty()
        {
            let Some((arg, typ)) = line.split_once(':') else {
                params.push((line.trim(), None));
                continue;
            };

            let typ = typ.trim();

            params.push((arg.trim(), Some(typ)));
        }
    }

    Some(params)
}

/// Extracts the docstring from a block of a function's contents.
pub fn extract_docstring(content: &str) -> Option<&str> {
    if let Some(stripped_content) = content.strip_prefix(r#"""""#) {
        let ending = stripped_content.find(r#"""""#)? + 6;

        Some(&content[0..ending])
    } else if let Some(stripped_content) = content.strip_prefix(r#"'''"#) {
        let ending = stripped_content.find(r#"'''"#)? + 6;

        Some(&content[0..ending])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google() {
        let docstring = r#"
            """Hey.

            Args:
                x (int): First var.
                y: Second var.
            """#;

        let args = parse_google_docstring(docstring, true).unwrap();

        assert_eq!(args[0].1.unwrap(), "int");
        assert_eq!(args[1].0, "y");

        assert_eq!(args.len(), 2);
    }

    #[test]
    fn numpy() {
        let docstring = r#"
            """Hey.

            Parameters
            ----------
            x: int
                First var.
            y
                Second var.
            """#;

        let args = parse_numpy_docstring(docstring, true).unwrap();

        assert_eq!(args[0].1.unwrap(), "int");
        assert_eq!(args[1].0, "y");

        assert_eq!(args.len(), 2);

        let docstring = r#"
            """Hey.

            Parameters
            """#;

        assert!(parse_numpy_docstring(docstring, true).is_none());
    }

    #[test]
    fn docstring_extraction() {
        let docstring = r#""""Hey.

            Args:
                x (int): First var.
                y: Second var.
            """
            x = 2
            y = 3 + 5"#;

        let docstring = extract_docstring(docstring).unwrap();

        assert_eq!(
            docstring,
            r#""""Hey.

            Args:
                x (int): First var.
                y: Second var.
            """"#
        );

        let docstring = r#"'''Hey.

            Args:
                x (int): First var.
                y: Second var.
            '''
            x = 2
            y = 3 + 5"#;

        let docstring = extract_docstring(docstring).unwrap();

        assert_eq!(
            docstring,
            r#"'''Hey.

            Args:
                x (int): First var.
                y: Second var.
            '''"#
        );

        let not_docstring = "Not a docstring.";

        assert!(extract_docstring(not_docstring).is_none());
    }
}
