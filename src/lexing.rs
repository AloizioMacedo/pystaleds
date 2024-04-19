use anyhow::{anyhow, Result};
use logos::{Lexer, Logos, Source};

use crate::ast_parsing::{FunctionInfo, FunctionLocation};

pub fn get_next_function_info<'a, 'b>(
    lexer: &mut Lexer<'a, Token>,
    params: &'b mut Vec<(&'a str, Option<&'a str>)>,
    skip_args_and_kwargs: bool,
) -> Option<FunctionInfo<'a, 'b>> {
    params.clear();

    while let Some(next) = lexer.next() {
        let Ok(Token::DefStart) = next else {
            continue;
        };

        lexer.next(); // Going to function name;
        let function_name = FunctionLocation::Name(lexer.slice());

        lexer.next(); // Going to first parenthesis;
        let mut current = lexer.next(); // Going to first variable;

        while let Some(Ok(Token::Text)) = current {
            let param_name = lexer.slice();

            let next = lexer.next();
            match next {
                Some(Ok(Token::Colon)) => {
                    lexer.next();

                    let (typ, finished_on) = extract_possibly_parenthesized_content(lexer).ok()?;

                    if param_name != "self"
                        && !(skip_args_and_kwargs
                            && (param_name.starts_with('*') || param_name.starts_with("**")))
                    {
                        params.push((param_name, Some(typ)));
                    }

                    match finished_on {
                        FinishedOn::Equals => {
                            lexer.next();
                            let (_, finished_on) =
                                extract_possibly_parenthesized_content(lexer).ok()?;

                            if let FinishedOn::ParClose = finished_on {
                                break;
                            }
                        }
                        FinishedOn::ParClose => {
                            break;
                        }
                        _ => (),
                    }
                }
                Some(Ok(Token::Equals)) => {
                    lexer.next();

                    let (_, finished_on) = extract_possibly_parenthesized_content(lexer).ok()?;

                    if param_name != "self"
                        && !(skip_args_and_kwargs
                            && (param_name.starts_with('*') || param_name.starts_with("**")))
                    {
                        params.push((param_name, None));
                    }

                    if let FinishedOn::ParClose = finished_on {
                        break;
                    }
                }
                _ => {
                    if param_name != "self"
                        && !(skip_args_and_kwargs
                            && (param_name.starts_with('*') || param_name.starts_with("**")))
                    {
                        params.push((param_name, None));
                    }
                }
            }

            current = lexer.next();
        }

        while let Some(ref t) = current {
            if let Ok(Token::Colon) = t {
                break;
            }

            current = lexer.next();
        }

        while let Some(t) = current {
            if let Ok(Token::Text) = t {
                let start = lexer.span().start;

                let slice = lexer.slice();

                let docstring = if slice.starts_with(r#"""""#) {
                    let end = lexer.source()[start + 3..]
                        .find(r#"""""#)
                        .expect("docstring should end");
                    Some(&lexer.source()[start..(start + end + 6)])
                } else if slice.starts_with(r#"'''"#) {
                    let end = lexer.source()[start + 3..]
                        .find(r#"'''"#)
                        .expect("docstring should end");
                    Some(&lexer.source()[start..(start + end + 6)])
                } else {
                    None
                };

                return Some(FunctionInfo {
                    params,
                    docstring,
                    function_name,
                });
            }

            current = lexer.next();
        }

        break;
    }

    None
}

enum FinishedOn {
    Comma,
    Equals,
    ParClose,
}

fn extract_possibly_parenthesized_content<'a>(
    lexer: &mut Lexer<'a, Token>,
) -> Result<(&'a str, FinishedOn)> {
    let mut count_par = 0;
    let mut count_brace = 0;
    let mut count_bracket = 0;

    let start = lexer.span().start;

    while let Some(Ok(tok)) = lexer.next() {
        match tok {
            Token::ParOpen => count_par += 1,
            Token::ParClose => {
                count_par -= 1;

                if count_par == -1 && count_brace == 0 && count_bracket == 0 {
                    let end = lexer.span().end - 1;
                    return lexer
                        .source()
                        .slice(start..end)
                        .map(|s| (s.trim(), FinishedOn::ParClose))
                        .ok_or(anyhow!(
            "could not extract type after variable. This is probably indicative of a syntax error"

                        ));
                }
            }
            Token::BraceOpen => count_brace += 1,
            Token::BraceClose => count_brace -= 1,
            Token::BracketOpen => count_bracket += 1,
            Token::BracketClose => count_bracket -= 1,
            Token::Equals => {
                if count_par == 0 && count_brace == 0 && count_bracket == 0 {
                    let end = lexer.span().end - 1;
                    return lexer
                        .source()
                        .slice(start..end)
                        .map(|s| (s.trim(), FinishedOn::Equals))
                        .ok_or(anyhow!(
            "could not extract type after variable. This is probably indicative of a syntax error"

                        ));
                }
            }
            Token::Comma => {
                if count_par == 0 && count_brace == 0 && count_bracket == 0 {
                    let end = lexer.span().end - 1;
                    return lexer
                        .source()
                        .slice(start..end)
                        .map(|s| (s.trim(), FinishedOn::Comma))
                        .ok_or(anyhow!(
            "could not extract type after variable. This is probably indicative of a syntax error"

                        ));
                }
            }
            _ => {}
        }
    }

    Err(anyhow!("reached end of lexing without enclosers"))
}

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"(\s+)|(\#.*\n)")] // Ignore this regex pattern between tokens
pub enum Token {
    // Tokens can be literal strings, of any length.
    #[token("def")]
    DefStart,

    #[token("(")]
    ParOpen,

    #[token(")")]
    ParClose,

    #[token("{")]
    BraceOpen,

    #[token("}")]
    BraceClose,

    #[token("[")]
    BracketOpen,

    #[token("]")]
    BracketClose,

    #[token(",")]
    Comma,

    #[token(":")]
    Colon,

    #[token("=")]
    Equals,

    // Or regular expressions.
    #[regex("[a-zA-Z0-9\'\"_|*]+")]
    Text,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let def = r#"def f(x, y, z):
    """Hello!""""#;

        let mut lex = Token::lexer(def);

        assert_eq!(lex.next(), Some(Ok(Token::DefStart)));
        assert_eq!(lex.slice(), "def");
        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "f");
        assert_eq!(lex.next(), Some(Ok(Token::ParOpen)));
        assert_eq!(lex.slice(), "(");
        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "x");
        assert_eq!(lex.next(), Some(Ok(Token::Comma)));
        assert_eq!(lex.slice(), ",");
        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "y");
        assert_eq!(lex.next(), Some(Ok(Token::Comma)));
        assert_eq!(lex.slice(), ",");
        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "z");
        assert_eq!(lex.next(), Some(Ok(Token::ParClose)));
        assert_eq!(lex.slice(), ")");
        assert_eq!(lex.next(), Some(Ok(Token::Colon)));
        assert_eq!(lex.slice(), ":");
    }

    #[test]
    fn test_get_function_info() {
        let def = r#"def f(x, y: int=2, z=323):
    """Hello!""""#;

        let mut lex = Token::lexer(def);

        let mut params = Vec::new();

        let function_info = get_next_function_info(&mut lex, &mut params, true).unwrap();

        assert_eq!(
            function_info.params,
            vec![("x", None), ("y", Some("int")), ("z", None)]
        );

        assert_eq!(function_info.function_name, FunctionLocation::Name("f"));

        assert_eq!(function_info.docstring.unwrap(), r#""""Hello!""""#);
    }

    #[test]
    fn test_get_function_info2() {
        let def = r#"def f(a, b: str = "wololo", c=323):
    """Hello!""""#;

        let mut lex = Token::lexer(def);

        let mut params = Vec::new();

        get_next_function_info(&mut lex, &mut params, true);

        assert_eq!(params, vec![("a", None), ("b", Some("str")), ("c", None)]);
    }

    #[test]
    fn test_get_function_info3() {
        let def = r#"x=2

def f(a,b,c):
    """Hello!"""

print(2)
yay = {"a": 2}

def g(x,y):
    """Hello!"""

    "#;

        let mut lex = Token::lexer(def);

        let mut params = Vec::new();

        get_next_function_info(&mut lex, &mut params, true);

        assert_eq!(params, vec![("a", None), ("b", None), ("c", None)]);

        get_next_function_info(&mut lex, &mut params, true);

        assert_eq!(params, vec![("x", None), ("y", None)]);
    }

    #[test]
    fn test_extracting_parenthesized_content() {
        let mut lex = Token::lexer("[c{df}] , aklsdfjla");
        let (result, _) = extract_possibly_parenthesized_content(&mut lex).unwrap();

        assert_eq!(result, "[c{df}]")
    }

    #[test]
    fn test_args() {
        let mut lex = Token::lexer(
            r#"
    def f(x, y, z, *args, a="oi"):
        """Hello.





        Nice spaces.


        Arguments:
            x:
            y:
            z:
            """
    "#,
        );
        let mut params = Vec::new();
        get_next_function_info(&mut lex, &mut params, true).unwrap();

        assert_eq!(
            params,
            vec![("x", None), ("y", None), ("z", None), ("a", None)]
        );

        let mut lex = Token::lexer(
            r#"
    def f(x, y, z, *args, a="oi"):
        """Hello.





        Nice spaces.


        Arguments:
            x:
            y:
            z:
            """
    "#,
        );

        let mut params = Vec::new();
        get_next_function_info(&mut lex, &mut params, false).unwrap();

        assert_eq!(
            params,
            vec![
                ("x", None),
                ("y", None),
                ("z", None),
                ("*args", None),
                ("a", None)
            ]
        );
    }
}
