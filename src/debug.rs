use tree_sitter::Node;

pub(crate) fn _debug_node(node: &Node, source_code: &str) {
    println!(
        "Kind: {}, Text: {}, Is Named: {}, Name: {}, Id: {}",
        node.kind(),
        node.utf8_text(source_code.as_bytes()).unwrap(),
        node.is_named(),
        node.grammar_name(),
        node.kind_id()
    );
}
