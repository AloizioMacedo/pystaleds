use tree_sitter::{Node, Parser, TreeCursor};

fn main() {
    let mut parser = Parser::new();

    parser
        .set_language(&tree_sitter_python::language())
        .expect("should be able to load Python grammar");
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
    println!(
        "Kind: {}, Text: {}, Is Named: {}, Name: {}",
        node.kind(),
        node.utf8_text(source_code.as_bytes()).unwrap(),
        node.is_named(),
        node.grammar_name()
    );

    get_docstring(node, source_code)
    // if node.kind() == "identifier" && node.parent().map(|n| n.kind() == "parameters") == Some(true)
    // {
    //     println!("{}", node.utf8_text(source_code.as_bytes()).unwrap());
    // }
}

fn get_docstring(node: &Node, source_code: &str) {
    let mut cur = node.walk();

    for child in node.children(&mut cur) {
        println!(
            "Child: {}",
            child.utf8_text(source_code.as_bytes()).unwrap()
        );
    }
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
    fn it_works() {
        let mut parser = get_parser();

        let source_code = "def add(x,y):\n    return x+y";
        let tree = parser.parse(source_code, None).unwrap();
        let root_node = tree.root_node();

        assert_eq!(root_node.start_position().column, 0);
        assert_eq!(root_node.end_position().column, 14);
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
}
