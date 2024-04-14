use crate::parsing::extract_docstring;
use tree_sitter::{Node, Point};

pub(crate) struct FunctionInfo<'a, 'b> {
    pub(crate) params: &'b [(&'a str, Option<&'a str>)],
    pub(crate) docstring: Option<&'a str>,
    pub(crate) start_position: Point,
}

pub(crate) fn get_function_signature<'a, 'b>(
    node: &Node,
    source_code: &'a str,
    params: &'b mut Vec<(&'a str, Option<&'a str>)>,
) -> Option<FunctionInfo<'a, 'b>> {
    if !node.kind().eq("function_definition") {
        return None;
    }

    let params_node = node.child_by_field_name("parameters")?;
    params.clear();

    for child in params_node.children(&mut params_node.walk()) {
        if child.utf8_text(source_code.as_bytes()).unwrap() == "self" {
            continue;
        }

        if child.kind() == "typed_parameter" || child.kind() == "typed_default_parameter" {
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
        } else if child.kind() == "default_parameter" {
            let (name, _) = child
                .utf8_text(source_code.as_bytes())
                .unwrap()
                .split_once('=')
                .unwrap();

            params.push((name, None));
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

    let content = block.utf8_text(source_code.as_bytes()).ok()?;
    let docstring = extract_docstring(content);

    let start_position = node.start_position();

    Some(FunctionInfo {
        params,
        docstring,
        start_position,
    })
}
