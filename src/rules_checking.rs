use std::path::Path;

use tracing::Level;
use tree_sitter::{Node, Parser, Tree, TreeCursor};

use crate::ast_parsing::{get_function_signature, FunctionInfo};
use crate::parsing::parse_google_docstring;

fn walk_rec<F>(cursor: &mut TreeCursor, closure: &mut F)
where
    F: FnMut(&Node),
{
    let node = cursor.node();

    closure(&node);

    if cursor.goto_first_child() {
        walk_rec(cursor, closure);
    }

    while cursor.goto_next_sibling() {
        walk_rec(cursor, closure);
    }

    cursor.goto_parent();
}

pub fn respects_rules(
    parser: &mut Parser,
    source_code: &str,
    old_tree: Option<&Tree>,
    path: Option<&Path>,
    succeed_if_no_docstring: bool,
    succeed_if_no_args_in_docstring: bool,
    docstring_should_always_be_typed: bool,
) -> bool {
    let tree = parser
        .parse(source_code, old_tree)
        .expect("parser should be ready to parse");

    let mut cursor = tree.walk();

    let mut success = true;

    walk_rec(&mut cursor, &mut |node| {
        let fs = get_function_signature(node, source_code);
        if let Some(info) = fs {
            if !is_function_info_valid(
                &info,
                path,
                succeed_if_no_docstring,
                succeed_if_no_args_in_docstring,
                docstring_should_always_be_typed,
            ) {
                success = false;
            }
        }
    });

    success
}

fn is_function_info_valid(
    info: &FunctionInfo,
    path: Option<&Path>,
    succeed_if_no_docstring: bool,
    succeed_if_no_args_in_docstring: bool,
    succeed_if_docstrings_are_not_typed: bool,
) -> bool {
    let path = path.map_or("".to_string(), |x| x.to_string_lossy().to_string() + ": ");

    let Some(docstring) = info.docstring else {
        if !succeed_if_no_docstring {
            tracing::event!(
                Level::ERROR,
                "{}Docstring missing at function starting on: {}",
                path,
                info.start_position
            );
        }

        return succeed_if_no_docstring;
    };

    let args_from_docstring = parse_google_docstring(docstring);

    let Some(args_from_docstring) = args_from_docstring else {
        if !succeed_if_no_args_in_docstring {
            tracing::event!(
                Level::ERROR,
                "{}Args missing from docstring at function starting on: {}",
                path,
                info.start_position
            );
        }

        return succeed_if_no_args_in_docstring;
    };

    if succeed_if_docstrings_are_not_typed {
        let is_valid = args_from_docstring.iter().zip(&info.params).all(
            |((param1, type1), (param2, type2))| match (type1, type2) {
                (Some(type1), Some(type2)) => param1 == param2 && type1 == type2,
                (_, _) => param1 == param2,
            },
        );

        if !is_valid {
            tracing::event!(
                Level::ERROR,
                "{}Docstring args not matching at function starting on {}",
                path,
                info.start_position
            );
        }

        is_valid
    } else {
        let is_valid = args_from_docstring == info.params;

        if !is_valid {
            tracing::event!(
                Level::ERROR,
                "Docstring args not matching at function start on  {:?}",
                info.start_position
            );
        }

        is_valid
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;
    use tree_sitter::Point;

    use super::*;

    fn get_parser() -> Parser {
        let mut parser = Parser::new();

        parser
            .set_language(&tree_sitter_python::language())
            .expect("should be able to load Python grammar");

        parser
    }

    #[test]
    #[traced_test]
    fn test_success_no_docstring() {
        let function_info = FunctionInfo {
            params: vec![("x", Some("int")), ("y", Some("str"))],
            docstring: None,
            start_position: Point { row: 0, column: 0 },
        };

        assert!(is_function_info_valid(
            &function_info,
            None,
            true,
            true,
            true
        ));

        assert!(!is_function_info_valid(
            &function_info,
            None,
            false,
            true,
            true
        ));
    }

    #[test]
    #[traced_test]
    fn test_out_of_order() {
        let function_info = FunctionInfo {
            params: vec![("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Args:
                    y: Nope.
                    x: Hehehe.
                """"#,
            ),
            start_position: Point { row: 0, column: 0 },
        };

        assert!(!is_function_info_valid(
            &function_info,
            None,
            true,
            true,
            true
        ));

        let function_info = FunctionInfo {
            params: vec![("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Args:
                    x: Hehehe.
                    y: Nope.
                """"#,
            ),
            start_position: Point { row: 0, column: 0 },
        };

        assert!(is_function_info_valid(
            &function_info,
            None,
            true,
            true,
            true
        ));
    }

    #[test]
    #[traced_test]
    fn test_check_function_info() {
        let function_info = FunctionInfo {
            params: vec![("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Args:
                    x (int): Hehehe.
                    y: Nope.
                """"#,
            ),
            start_position: Point { row: 0, column: 0 },
        };

        assert!(is_function_info_valid(
            &function_info,
            None,
            false,
            false,
            true
        ));
    }

    #[test]
    #[traced_test]
    fn test_file() {
        let mut parser = get_parser();

        let source_code = r#"def add(x: int,y):
    """This is a docstring."""
    return x+y

def sub(x, y):
    """This is a multi-line docstring.

    And this is the rest.
    Args:
        x (int): Hehehe.
        y (int): Nope.
    """
    return x-y

def other_func(x,y,z):
    """This is just a throw-away string!"""
    return x+y+2*z
"#;

        let x = respects_rules(&mut parser, source_code, None, None, false, true, true);

        assert!(x);
    }

    #[test]
    #[traced_test]
    fn test_nesting() {
        let mut parser = get_parser();

        let source_code = r#"def add(x: int,y):
    """This is a docstring."""
    def sub(x: int, y: int):
        """This is a nested docstring.

        Args:
            x (int): Haha.
            y (str): Should error!
        """
        return x-y
    return x+y
"#;

        let x = respects_rules(&mut parser, source_code, None, None, true, true, false);

        assert!(!x);

        let source_code = r#"def add(x: int,y):
    """This is a docstring."""
    def sub(x: int, y: int):
        """This is a nested docstring.

        Args:
            x (int): Haha.
            y (int): Should error!
        """
        return x-y
    return x+y
"#;

        let x = respects_rules(&mut parser, source_code, None, None, true, true, false);

        assert!(x);
    }

    #[test]
    fn test_from_path() {
        let mut parser = get_parser();

        let path = std::path::PathBuf::from("test_folder/test.py");
        let source_code = std::fs::read_to_string("test_folder/test.py").unwrap();

        assert!(respects_rules(
            &mut parser,
            &source_code,
            None,
            Some(&path),
            true,
            true,
            true,
        ));

        let path = std::path::PathBuf::from("test_folder/test_cp.py");
        let source_code = std::fs::read_to_string("test_folder/test_cp.py").unwrap();

        assert!(!respects_rules(
            &mut parser,
            &source_code,
            None,
            Some(&path),
            true,
            true,
            true,
        ));
    }
}
