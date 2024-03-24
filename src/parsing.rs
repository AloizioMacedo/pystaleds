use std::collections::HashMap;

fn parse_google_docstring(text: &str) -> Option<HashMap<&str, Option<&str>>> {
    let (_, args) = text.split_once("Args:\n")?;

    let first_line = args.lines().next()?;

    let indentation = first_line.chars().take_while(|c| c.is_whitespace()).count();

    let mut params = HashMap::new();

    for line in args.lines() {
        if line.chars().take(indentation).all(|c| c.is_whitespace())
            && line.chars().nth(indentation).map(|c| !c.is_whitespace()) == Some(true)
        {
            let Some((arg, _)) = line.split_once(':') else {
                continue;
            };

            let arg = arg.trim();

            let Some((name, typ)) = arg.split_once(' ') else {
                params.insert(arg, None);
                continue;
            };

            let typ = typ.trim_start_matches('(').trim_end_matches(')');

            params.insert(name, Some(typ));
        }
    }

    Some(params)
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

        assert_eq!(args.get("x").unwrap().unwrap(), "int");
        assert!(args.get("y").unwrap().is_none());
    }
}
