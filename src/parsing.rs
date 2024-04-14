pub fn parse_google_docstring<'a>(
    text: &'a str,
    buffer: &mut Vec<(&'a str, Option<&'a str>)>,
) -> Option<()> {
    let (_, mut args) = text.split_once("Args:\n")?;

    if let Some(c) = args.find("Returns:\n") {
        args = &args[..c];
    };

    let first_line = args.lines().next()?;

    let indentation = first_line.chars().take_while(|c| c.is_whitespace()).count();

    for line in args.lines() {
        if line.chars().take(indentation).all(|c| c.is_whitespace())
            && line.chars().nth(indentation).map(|c| !c.is_whitespace()) == Some(true)
        {
            let Some((arg, _)) = line.split_once(':') else {
                continue;
            };

            let arg = arg.trim();

            let Some((name, typ)) = arg.split_once(' ') else {
                buffer.push((arg, None));
                continue;
            };

            let typ = typ.trim_start_matches('(').trim_end_matches(')');

            buffer.push((name, Some(typ)));
        }
    }

    Some(())
}

pub fn parse_numpy_docstring<'a>(
    text: &'a str,
    buffer: &mut Vec<(&'a str, Option<&'a str>)>,
) -> Option<()> {
    let (_, mut args) = text.split_once("Parameters\n")?;

    if let Some(c) = args.find("Returns\n") {
        args = &args[..c];
    };

    let first_line = args.lines().nth(1)?;

    let indentation = first_line.chars().take_while(|c| c.is_whitespace()).count();

    for line in args.lines().skip(1) {
        if line.chars().take(indentation).all(|c| c.is_whitespace())
            && line.chars().nth(indentation).map(|c| !c.is_whitespace()) == Some(true)
            && !line.trim().trim_end_matches(&['\'', '\"']).is_empty()
        {
            let Some((arg, typ)) = line.split_once(':') else {
                buffer.push((line.trim(), None));
                continue;
            };

            let typ = typ.trim();

            buffer.push((arg.trim(), Some(typ)));
        }
    }

    Some(())
}

pub fn extract_docstring(content: &str) -> Option<&str> {
    if !content.starts_with(r#"""""#) {
        return None;
    }

    let ending = content[3..].find(r#"""""#)? + 6;

    Some(&content[0..ending])
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
        let mut args = Vec::new();

        parse_google_docstring(docstring, &mut args).unwrap();

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
        let mut buffer = Vec::new();

        parse_numpy_docstring(docstring, &mut buffer).unwrap();

        assert_eq!(buffer[0].1.unwrap(), "int");
        assert_eq!(buffer[1].0, "y");

        assert_eq!(buffer.len(), 2);

        let docstring = r#"
            """Hey.

            Parameters
            """#;

        buffer.clear();

        assert!(parse_numpy_docstring(docstring, &mut buffer).is_none());
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

        let not_docstring = "Not a docstring.";

        assert!(extract_docstring(not_docstring).is_none());
    }
}
