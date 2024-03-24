use tree_sitter::{Node, Parser, TreeCursor};

fn main() {
    let mut parser = Parser::new();

    parser
        .set_language(&tree_sitter_python::language())
        .expect("should be able to load Python grammar");
}

fn walk_rec(cursor: &mut TreeCursor, closure: fn(&Node) -> ()) {
    let node = cursor.node();

    closure(&node);

    while cursor.goto_next_sibling() {
        let node = cursor.node();

        closure(&node);
    }

    while cursor.goto_first_child() {
        walk_rec(cursor, closure);
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

        let source_code = r#"def add(x,y):
    return x+y

def sub(x,y):
    return x-y"#;
        let tree = parser.parse(source_code, None).unwrap();
        let root_node = tree.root_node();

        walk_rec(&mut root_node.walk(), |node| println!("{:?}", node));
    }
}
