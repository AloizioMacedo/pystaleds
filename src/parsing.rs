pub fn parse_google_docstring(text: &str) -> Option<Vec<(&str, Option<&str>)>> {
    let (_, args) = text.split_once("Args:\n")?;

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

fn extract_docstring(content: &str) -> Option<&str> {
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
    fn it_works() {
        let docstring = r#"
            """Hey.

            Args:
                x (int): First var.
                y: Second var.
            """#;

        let args = parse_google_docstring(docstring).unwrap();

        assert_eq!(args[0].1.unwrap(), "int");
        assert_eq!(args[1].0, "y");
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
    }
}
