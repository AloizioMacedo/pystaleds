use std::fmt::Display;

use crate::parsing::extract_docstring;
use tree_sitter::Node;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum FunctionLocation<'a> {
    Name(&'a str),
    Row(usize),
}

impl<'a> Display for FunctionLocation<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FunctionLocation::Name(x) => f.write_str(x),
            FunctionLocation::Row(x) => f.write_str(&format!("{}", x)),
        }
    }
}

/// Information about a function's signature and docstring.
pub(crate) struct FunctionInfo<'a, 'b> {
    pub(crate) params: &'b [(&'a str, Option<&'a str>)],
    pub(crate) docstring: Option<&'a str>,
    pub(crate) function_name: FunctionLocation<'a>,
}

/// Extracts function information from a node if it is a function definition.
///
/// Uses a buffered params vector for performance, instead of allocating a new one
/// every time.
#[inline]
pub(crate) fn get_function_signature<'a, 'b>(
    node: &Node,
    source_code: &'a str,
    params: &'b mut Vec<(&'a str, Option<&'a str>)>,
) -> Option<FunctionInfo<'a, 'b>> {
    if !node.kind().eq("function_definition") {
        return None;
    }

    let function_name = FunctionLocation::Row(node.start_position().row);

    let params_node = node.child_by_field_name("parameters")?;
    params.clear();

    let mut cursor = params_node.walk();

    for child in params_node.children(&mut cursor) {
        let text = child
            .utf8_text(source_code.as_bytes())
            .expect("should be valid utf-8");

        if text == "self" {
            continue;
        }

        if child.kind() == "typed_parameter" || child.kind() == "typed_default_parameter" {
            let mut identifier = None;
            let mut typ = None;

            let mut d = child.walk();

            for inner_child in child.children(&mut d) {
                let text_of_inner_child = inner_child
                    .utf8_text(source_code.as_bytes())
                    .expect("should be valid utf-8");

                if inner_child.kind() == "identifier" {
                    identifier = Some(text_of_inner_child);
                } else if inner_child.kind() == "type" {
                    typ = Some(text_of_inner_child);
                }
            }

            if let (Some(identifier), Some(typ)) = (identifier, typ) {
                params.push((identifier, Some(typ)));
            }
        } else if child.kind() == "identifier" {
            params.push((text, None));
        } else if child.kind() == "default_parameter" {
            let (name, _) = text
                .split_once('=')
                .expect("parameter with default value should have '=' in the text");

            params.push((name, None));
        }
    }

    let block = node.children(&mut cursor).find(|c| c.kind() == "block")?;

    let content = block.utf8_text(source_code.as_bytes()).ok()?;
    let docstring = extract_docstring(content);

    Some(FunctionInfo {
        params,
        docstring,
        function_name,
    })
}
