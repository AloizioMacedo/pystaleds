use std::path::Path;

use clap::ValueEnum;
use logos::Lexer;
use tracing::Level;
use tree_sitter::{Node, Parser, Tree, TreeCursor};

use crate::ast_parsing::{get_function_signature, FunctionInfo};
use crate::lexing::get_next_function_info;
use crate::parsing::{parse_google_docstring, parse_numpy_docstring};

#[derive(Default, Clone, Copy, ValueEnum)]
pub enum DocstringStyle {
    Google,
    Numpy,
    #[default]
    AutoDetect,
}

/// Walks recursively through a tree applying a closure on each node.
fn walk_rec<F>(cursor: &mut TreeCursor, closure: &mut F)
where
    for<'a> F: FnMut(&Node),
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

/// Checks if the source code respects the specified rules.
#[allow(clippy::too_many_arguments)]
pub fn respects_rules(
    parser: &mut Parser,
    source_code: &str,
    old_tree: Option<&Tree>,
    path: Option<&Path>,
    break_on_empty_line: bool,
    succeed_if_no_docstring: bool,
    succeed_if_no_args_in_docstring: bool,
    succeed_if_docstrings_are_not_typed: bool,
    skip_args_and_kwargs: bool,
    docstyle: DocstringStyle,
) -> bool {
    let tree = parser
        .parse(source_code, old_tree)
        .expect("parser should be ready to parse");

    let mut cursor = tree.walk();

    let mut success = true;
    let mut params = Vec::with_capacity(8);

    walk_rec(&mut cursor, &mut |node| {
        let fs = get_function_signature(node, source_code, &mut params);
        if let Some(info) = fs {
            if !is_function_info_valid(
                &info,
                path,
                break_on_empty_line,
                succeed_if_no_docstring,
                succeed_if_no_args_in_docstring,
                succeed_if_docstrings_are_not_typed,
                skip_args_and_kwargs,
                docstyle,
            ) {
                success = false;
            }
        }
    });

    success
}

/// Checks if the source code respects the specified rules.
#[allow(clippy::too_many_arguments)]
pub fn respects_rules_through_lexing(
    source_code: &str,
    path: Option<&Path>,
    break_on_empty_line: bool,
    succeed_if_no_docstring: bool,
    succeed_if_no_args_in_docstring: bool,
    succeed_if_docstrings_are_not_typed: bool,
    skip_args_and_kwargs: bool,
    docstyle: DocstringStyle,
) -> bool {
    let mut lexer = Lexer::new(source_code);

    let mut success = true;
    let mut params = Vec::with_capacity(8);

    while let Some(info) = get_next_function_info(&mut lexer, &mut params, skip_args_and_kwargs) {
        if !is_function_info_valid(
            &info,
            path,
            break_on_empty_line,
            succeed_if_no_docstring,
            succeed_if_no_args_in_docstring,
            succeed_if_docstrings_are_not_typed,
            skip_args_and_kwargs,
            docstyle,
        ) {
            success = false;
        }
    }

    success
}

/// Checks if a given function respects the specified rules.
#[allow(clippy::too_many_arguments)]
fn is_function_info_valid(
    info: &FunctionInfo,
    path: Option<&Path>,
    break_on_empty_line: bool,
    succeed_if_no_docstring: bool,
    succeed_if_no_args_in_docstring: bool,
    succeed_if_docstrings_are_not_typed: bool,
    skip_args_and_kwargs: bool,
    docstyle: DocstringStyle,
) -> bool {
    let path = path.map_or("".to_string(), |x| x.to_string_lossy().to_string() + ": ");

    let Some(docstring) = info.docstring else {
        if !succeed_if_no_docstring {
            tracing::event!(
                Level::ERROR,
                "{}`{}`: Docstring missing",
                path,
                info.function_name
            );
        }

        return succeed_if_no_docstring;
    };

    let args_from_docstring = match docstyle {
        DocstringStyle::Google => {
            parse_google_docstring(docstring, break_on_empty_line, skip_args_and_kwargs)
        }
        DocstringStyle::Numpy => {
            parse_numpy_docstring(docstring, break_on_empty_line, skip_args_and_kwargs)
        }
        DocstringStyle::AutoDetect => {
            parse_google_docstring(docstring, break_on_empty_line, skip_args_and_kwargs).or(
                parse_numpy_docstring(docstring, break_on_empty_line, skip_args_and_kwargs),
            )
        }
    };

    let Some(args_from_docstring) = args_from_docstring else {
        if !succeed_if_no_args_in_docstring {
            tracing::event!(
                Level::ERROR,
                "{}`{}`: Args missing from docstring",
                path,
                info.function_name
            );
        }

        return succeed_if_no_args_in_docstring;
    };

    if succeed_if_docstrings_are_not_typed {
        let is_valid = if args_from_docstring.len() == info.params.len() {
            args_from_docstring
                .iter()
                .zip(info.params)
                .all(|((param1, type1), (param2, type2))| match (type1, type2) {
                    (Some(type1), Some(type2)) => param1 == param2 && type1 == type2,
                    (_, _) => param1 == param2,
                })
        } else {
            false
        };

        if !is_valid {
            tracing::event!(
                Level::ERROR,
                "{}`{}`: Args from function: {:?}. Args from docstring: {:?}",
                path,
                info.function_name,
                info.params,
                args_from_docstring,
            );
        }

        is_valid
    } else {
        let is_valid = args_from_docstring == info.params;

        if !is_valid {
            tracing::event!(
                Level::ERROR,
                "Docstring args not matching at function {}",
                info.function_name
            );
        }

        is_valid
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::ast_parsing::FunctionLocation;

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
            params: &[("x", Some("int")), ("y", Some("str"))],
            docstring: None,
            function_name: FunctionLocation::Name(""),
        };

        assert!(is_function_info_valid(
            &function_info,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::AutoDetect
        ));

        assert!(!is_function_info_valid(
            &function_info,
            None,
            false,
            false,
            true,
            true,
            true,
            DocstringStyle::AutoDetect
        ));
    }

    #[test]
    #[traced_test]
    fn test_out_of_order() {
        let function_info = FunctionInfo {
            params: &[("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Args:
                    y: Nope.
                    x: Hehehe.
                """"#,
            ),
            function_name: FunctionLocation::Name(""),
        };

        assert!(!is_function_info_valid(
            &function_info,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        let function_info = FunctionInfo {
            params: &[("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Args:
                    x: Hehehe.
                    y: Nope.
                """"#,
            ),
            function_name: FunctionLocation::Name(""),
        };

        assert!(is_function_info_valid(
            &function_info,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        let function_info = FunctionInfo {
            params: &[("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Parameters
                ----------
                y
                    Nope
                x
                    Hehehe
                """"#,
            ),
            function_name: FunctionLocation::Name(""),
        };

        assert!(!is_function_info_valid(
            &function_info,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Numpy
        ));

        let function_info = FunctionInfo {
            params: &[("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Parameters
                ----------
                x
                    Hehehe
                y
                    Nope.

                Returns
                ------
                ...
                """"#,
            ),
            function_name: FunctionLocation::Name(""),
        };

        assert!(is_function_info_valid(
            &function_info,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Numpy
        ));
    }

    #[test]
    #[traced_test]
    fn test_check_function_info() {
        let function_info = FunctionInfo {
            params: &[("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Args:
                    x (int): Hehehe.
                    y: Nope.

                Returns:
                    ...
                """"#,
            ),
            function_name: FunctionLocation::Name(""),
        };

        assert!(is_function_info_valid(
            &function_info,
            None,
            false,
            false,
            false,
            true,
            true,
            DocstringStyle::Google
        ));

        assert!(is_function_info_valid(
            &function_info,
            None,
            false,
            false,
            false,
            true,
            true,
            DocstringStyle::AutoDetect
        ));
    }

    #[test]
    #[traced_test]
    fn ignore_edge_case() {
        let function_info = FunctionInfo {
            params: &[("x", Some("int")), ("y", Some("str"))],
            docstring: Some(
                r#"
                """
                Hello!

                Args:
                    x (int): Hehehe.
                    KLJZXKLC
                    y: Nope.

                Returns:
                    ...
                """"#,
            ),
            function_name: FunctionLocation::Name(""),
        };

        assert!(is_function_info_valid(
            &function_info,
            None,
            false,
            false,
            false,
            true,
            true,
            DocstringStyle::Google
        ));
    }

    #[test]
    #[traced_test]
    fn test_more_and_less_args() {
        let mut parser = get_parser();

        let source_code = r#"def sub(x, y):
    """This is a multi-line docstring.

    And this is the rest.
    Args:
        x: Hehehe.
        y: Nope.
    """
    return x-y
"#;

        assert!(respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            true,
            false,
            DocstringStyle::Google,
        ));

        assert!(respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google,
        ));

        let source_code = r#"def sub(x, y):
    """This is a multi-line docstring.

    And this is the rest.
    Args:
        y: Nope.
    """
    return x-y
"#;

        assert!(!respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google,
        ));

        assert!(!respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google,
        ));

        let source_code = r#"def sub(x, y):
    """This is a multi-line docstring.

    And this is the rest.
    Args:
        x: Hehe.
        y: Nope.
        z: This shouldn't exist.
    """
    return x-y
"#;

        assert!(!respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google,
        ));

        assert!(!respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google,
        ));
    }

    #[test]
    #[traced_test]
    fn missing_args_docstring() {
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

        assert!(!respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            false,
            false,
            true,
            true,
            DocstringStyle::Google,
        ));

        assert!(!respects_rules_through_lexing(
            source_code,
            None,
            false,
            false,
            false,
            true,
            true,
            DocstringStyle::Google,
        ))
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

        assert!(respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            false,
            true,
            true,
            true,
            DocstringStyle::Google,
        ));

        assert!(respects_rules_through_lexing(
            source_code,
            None,
            false,
            false,
            true,
            true,
            true,
            DocstringStyle::Google,
        ));
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

        assert!(!respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            false,
            true,
            DocstringStyle::Google,
        ));

        assert!(!respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            false,
            true,
            DocstringStyle::Google,
        ));

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

        assert!(respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            false,
            true,
            DocstringStyle::Google,
        ));

        assert!(respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            false,
            true,
            DocstringStyle::Google,
        ));
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
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        assert!(respects_rules_through_lexing(
            &source_code,
            Some(&path),
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        let path = std::path::PathBuf::from("test_folder/test_cp.py");
        let source_code = std::fs::read_to_string("test_folder/test_cp.py").unwrap();

        assert!(!respects_rules(
            &mut parser,
            &source_code,
            None,
            Some(&path),
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        assert!(!respects_rules_through_lexing(
            &source_code,
            Some(&path),
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));
    }

    #[test]
    fn test_untyped_default_param() {
        let mut parser = get_parser();
        let source_code = r#"def f(a, x=2):
    """
    Hey.

    Args:
        a: _description
        x (int): _description_

    Returns:
        _type_: _description_
    """
    "Oi"
    return x
"#;
        assert!(respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        assert!(respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));
    }

    #[test]
    fn test_break_on_new_line() {
        let mut parser = get_parser();
        let source_code = r#"def f(a, x=2):
    """
    Hey.

    Args:
        a: _description
        x (int): _description_
            asjkldasjkld

        askldjfasjkldf: alskdj
    """
    "Oi"
    return x
"#;
        assert!(!respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        assert!(!respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        assert!(respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            true,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        assert!(respects_rules_through_lexing(
            source_code,
            None,
            true,
            true,
            true,
            true,
            true,
            DocstringStyle::Google
        ));

        let source_code = r#"def f(a, x=2):  # Comment to try and screw up the lexer.
    """
    Hey.

    Parameters
    ----------
    a
    x : int
        Hohohoh

    See also
    --------
    Something else
    """
    return x
"#;
        assert!(!respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Numpy
        ));

        assert!(!respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::Numpy
        ));

        assert!(respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            true,
            true,
            true,
            true,
            true,
            DocstringStyle::Numpy
        ));

        assert!(respects_rules_through_lexing(
            source_code,
            None,
            true,
            true,
            true,
            true,
            true,
            DocstringStyle::Numpy
        ));
    }

    #[test]
    fn test_real_code() {
        let mut parser = get_parser();
        let source_code = r#"@dataclass
        class TimeSeries:
            def __init__(
                self,
                raw_data: pd.DataFrame,
                cnpj: str = "",
                min_date: Optional[str] = None,
                max_date: Optional[str] = None,
            ):
                self.raw_data = raw_data
                self.cnpj = cnpj

                min_date = self.raw_data[DT].min()
                max_date = self.raw_data[DT].max()

                self._min_date = min_date if min_date is not None else min_date
                self._max_date = max_date if max_date is not None else max_date

                self._calculation_cache: Dict[Tuple[str, str], float] = {}

            def filter_ts(self, from_date: str, to_date: str):
                if self._min_date == from_date and self._max_date == to_date:
                    return

                self.raw_data = self.raw_data[
                    (self.raw_data[DT] >= from_date) & (self.raw_data[DT] <= to_date)
                ].reset_index()

                self._min_date = from_date
                self._max_date = to_date

            def calculate_value_at_end(
                self,
                from_date: str,
                to_date: str,
                initial_investiment: float = 1.0,
            ) -> float:
                if (result := self._calculation_cache.get((from_date, to_date))) is not None:
                    return result * initial_investiment

                values = self.raw_data[
                    (self.raw_data[DT] >= from_date) & (self.raw_data[DT] <= to_date)
                ][VALUES]

                product: float = values.product()  # type: ignore

                self._calculation_cache[(from_date, to_date)] = product

                return product * initial_investiment

            def average(
                self,
                from_date: Optional[str] = None,
                to_date: Optional[str] = None,
            ) -> float:
                from_date = from_date if from_date is not None else self._min_date
                to_date = to_date if to_date is not None else self._max_date

                values = self.raw_data[
                    (self.raw_data[DT] >= from_date) & (self.raw_data[DT] <= to_date)
                ][VALUES]

                mean = values.mean()
                return mean  # type: ignore

            def variance(
                self,
                from_date: Optional[str] = None,
                to_date: Optional[str] = None,
            ) -> float:
                """Hello.

                Args:
                    from_date: Hohoho.
                    to_date: None,

                """
                from_date = from_date if from_date is not None else self._min_date
                to_date = to_date if to_date is not None else self._max_date

                values = self.raw_data[
                    (self.raw_data[DT] >= from_date) & (self.raw_data[DT] <= to_date)
                ][VALUES]

                var: float = values.var()  # type: ignore
                return var

            def geometric_mean(self, from_date: Optional[str], to_date: Optional[str]) -> float:
                from_date = from_date if from_date is not None else self._min_date
                to_date = to_date if to_date is not None else self._max_date

                values = self.raw_data[
                    (self.raw_data[DT] >= from_date) & (self.raw_data[DT] <= to_date)
                ][VALUES]

                product: float = values.product()  # type: ignore
                geo_mean = (product) ** (1 / len(values))

                return geo_mean

            def correlation(
                self,
                other: TimeSeries,
                from_date: Optional[str],
                to_date: Optional[str],
            ) -> float:
                from_date = from_date if from_date is not None else self._min_date
                to_date = to_date if to_date is not None else self._max_date

                values1 = self.raw_data[
                    (self.raw_data[DT] >= from_date) & (self.raw_data[DT] <= to_date)
                ][VALUES]

                values2 = other.raw_data[
                    (other.raw_data[DT] >= from_date) & (other.raw_data[DT] <= to_date)
                ][VALUES]

                return np.corrcoef(values1, values2)[0, 1]

        "#;

        assert!(respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::AutoDetect
        ));

        assert!(respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::AutoDetect
        ));
    }

    #[test]
    fn test_real_code2() {
        let mut parser = get_parser();

        let source_code = r#"
        @final
        @staticmethod
        def _validate_subplots_kwarg(
            subplots: bool | Sequence[Sequence[str]], data: Series | DataFrame, kind: str
        ) -> bool | list[tuple[int, ...]]:
            """
            Validate the subplots parameter

            - check type and content
            - check for duplicate columns
            - check for invalid column names
            - convert column names into indices
            - add missing columns in a group of their own
            See comments in code below for more details.

            Parameters
            ----------
            subplots : subplots parameters as passed to PlotAccessor

            Returns
            -------
            validated subplots : a bool or a list of tuples of column indices. Columns
            in the same tuple will be grouped together in the resulting plot.
            """"#;

        assert!(!respects_rules(
            &mut parser,
            source_code,
            None,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::AutoDetect
        ));

        assert!(!respects_rules_through_lexing(
            source_code,
            None,
            false,
            true,
            true,
            true,
            true,
            DocstringStyle::AutoDetect
        ));
    }
}
