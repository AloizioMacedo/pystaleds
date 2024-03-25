mod parsing;

use tree_sitter::{Node, Parser, TreeCursor};

use crate::parsing::{extract_docstring, parse_google_docstring};

fn main() {
    let mut parser = Parser::new();

    parser
        .set_language(&tree_sitter_python::language())
        .expect("should be able to load Python grammar");
}

#[derive(Debug)]
struct FunctionInfo<'a> {
    params: Vec<(&'a str, Option<&'a str>)>,
    docstring: Option<&'a str>,
}

fn walk_rec<F>(cursor: &mut TreeCursor, closure: &F)
where
    F: Fn(&Node),
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

fn print_param(node: &Node, source_code: &str) {
    get_function_signature(node, source_code);
    // if node.kind() == "identifier" && node.parent().map(|n| n.kind() == "parameters") == Some(true)
    // {
    //     println!("{}", node.utf8_text(source_code.as_bytes()).unwrap());
    // }
}

fn debug_node(node: &Node, source_code: &str) {
    println!(
        "Kind: {}, Text: {}, Is Named: {}, Name: {}, Id: {}",
        node.kind(),
        node.utf8_text(source_code.as_bytes()).unwrap(),
        node.is_named(),
        node.grammar_name(),
        node.kind_id()
    );
}

fn get_function_signature<'a>(node: &Node, source_code: &'a str) -> Option<FunctionInfo<'a>> {
    if !node.kind().eq("function_definition") {
        return None;
    }

    let params_node = node.child_by_field_name("parameters")?;

    let mut params = Vec::new();

    for child in params_node.children(&mut params_node.walk()) {
        if child.kind() == "typed_parameter" {
            let mut identifier = None;
            let mut typ = None;

            let mut d = child.walk();

            for child in child.children(&mut d) {
                if child.kind() == "identifier" {
                    identifier = Some(child.utf8_text(source_code.as_bytes()).unwrap());
                } else if child.kind() == "type" {
                    typ = Some(child.utf8_text(source_code.as_bytes()).unwrap());
                }
            }

            if let (Some(identifier), Some(typ)) = (identifier, typ) {
                params.push((identifier, Some(typ)));
            }
        } else if child.kind() == "identifier" {
            params.push((child.utf8_text(source_code.as_bytes()).unwrap(), None));
        }
    }

    let mut block = None;
    for child in node.children(&mut node.walk()) {
        if child.kind() == "block" {
            block = Some(child);
            break;
        }
    }

    let block = block?;

    eprintln!("{:?}", block);
    let content = block.utf8_text(source_code.as_bytes()).ok()?;
    let docstring = extract_docstring(content);

    Some(FunctionInfo { params, docstring })
}

fn check_function_info(info: &FunctionInfo) -> bool {
    let Some(docstring) = info.docstring else {
        return false;
    };

    let args_from_docstring = parse_google_docstring(docstring);

    let Some(args_from_docstring) = args_from_docstring else {
        return false;
    };

    args_from_docstring == info.params
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_parser() -> Parser {
        let mut parser = Parser::new();

        parser
            .set_language(&tree_sitter_python::language())
            .expect("should be able to load Python grammar");

        parser
    }

    #[test]
    fn recursive_walk() {
        let mut parser = get_parser();

        let source_code = r#"def add(x: int,y):
    """This is a docstring."""
    return x+y

def sub(x,y):
    """This is a multi-line docstring.

    And this is the rest.
    Args:
        x (int): Hehehe.
        y (int): Nope.
    """
    return x-y

def other_func(x,y,z):
    "This is just a throw-away string!"
    return x+y+2*z
"#;
        let tree = parser.parse(source_code, None).unwrap();
        let root_node = tree.root_node();

        let mut cursor = root_node.walk();

        walk_rec(&mut cursor, &|node| print_param(node, source_code));
    }

    #[test]
    fn test_check_function_info() {
        let function_info = FunctionInfo {
            params: vec![("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Args:
                    x (int): Hehehe.
                    y (str): Nope.
                """"#,
            ),
        };

        assert!(check_function_info(&function_info));
    }
}
