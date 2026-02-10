//! SPARQL CONSTRUCT query parser using nom.

use crate::ast::*;
use crate::error::{Error, Result};
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until, take_while, take_while1},
    character::complete::{char, multispace0, multispace1, one_of},
    combinator::{map, opt, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated},
};
use std::collections::HashMap;

/// Parser for SPARQL CONSTRUCT queries.
pub struct Parser;

impl Parser {
    /// Parse a complete SPARQL CONSTRUCT query.
    pub fn parse(input: &str) -> Result<Query> {
        match parse_query(input) {
            Ok(("", query)) => Ok(query),
            Ok((remaining, _)) => Err(Error::parse_error(
                input.len() - remaining.len(),
                format!(
                    "Unexpected input remaining: {}",
                    &remaining[..20.min(remaining.len())]
                ),
            )),
            Err(e) => {
                let position = match e {
                    nom::Err::Error(e) | nom::Err::Failure(e) => input.len() - e.input.len(),
                    nom::Err::Incomplete(_) => input.len(),
                };
                Err(Error::parse_error(position, "Parse failed"))
            }
        }
    }
}

/// Parse whitespace (including comments).
fn ws(input: &str) -> IResult<&str, &str> {
    multispace0(input)
}

/// Parse required whitespace.
fn ws1(input: &str) -> IResult<&str, &str> {
    multispace1(input)
}

/// Parse a complete query.
fn parse_query(input: &str) -> IResult<&str, Query> {
    let (input, _) = ws(input)?;
    let (input, prefixes) = parse_prefixes(input)?;
    let (input, _) = ws(input)?;
    let (input, construct) = parse_construct_clause(input)?;
    let (input, _) = ws(input)?;
    let (input, where_clause) = parse_where_clause(input)?;
    let (input, _) = ws(input)?;

    Ok((
        input,
        Query {
            prefixes,
            construct,
            where_clause,
        },
    ))
}

/// Parse PREFIX declarations.
fn parse_prefixes(input: &str) -> IResult<&str, HashMap<String, String>> {
    let (input, prefix_list) = many0(terminated(parse_prefix, ws))(input)?;
    Ok((input, prefix_list.into_iter().collect()))
}

/// Parse a single PREFIX declaration.
fn parse_prefix(input: &str) -> IResult<&str, (String, String)> {
    let (input, _) = tag_no_case("PREFIX")(input)?;
    let (input, _) = ws1(input)?;
    let (input, prefix) = parse_prefix_name(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(':')(input)?;
    let (input, _) = ws(input)?;
    let (input, iri) = parse_iri_ref(input)?;
    let (input, _) = ws(input)?;

    Ok((input, (prefix, iri)))
}

/// Parse a prefix name (alphanumeric identifier).
fn parse_prefix_name(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        |s: &str| s.to_string(),
    )(input)
}

/// Parse an IRI reference <...>.
fn parse_iri_ref(input: &str) -> IResult<&str, String> {
    delimited(
        char('<'),
        map(take_until(">"), |s: &str| s.to_string()),
        char('>'),
    )(input)
}

/// Parse the CONSTRUCT clause.
fn parse_construct_clause(input: &str) -> IResult<&str, ConstructTemplate> {
    let (input, _) = tag_no_case("CONSTRUCT")(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('{')(input)?;
    let (input, _) = ws(input)?;
    let (input, patterns) = parse_triple_patterns(input)?;
    let (input, _) = ws(input)?;
    // Consume optional trailing period
    let (input, _) = opt(char('.'))(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('}')(input)?;

    Ok((input, ConstructTemplate { patterns }))
}

/// Parse the WHERE clause.
fn parse_where_clause(input: &str) -> IResult<&str, GraphPattern> {
    let (input, _) = tag_no_case("WHERE")(input)?;
    let (input, _) = ws(input)?;
    parse_graph_pattern(input)
}

/// Parse a graph pattern.
fn parse_graph_pattern(input: &str) -> IResult<&str, GraphPattern> {
    let (input, _) = char('{')(input)?;
    let (input, _) = ws(input)?;
    let (input, pattern) = parse_group_graph_pattern(input)?;
    let (input, _) = ws(input)?;
    // Consume optional trailing period
    let (input, _) = opt(char('.'))(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, pattern))
}

/// Parse a group graph pattern (sequence of patterns).
fn parse_group_graph_pattern(input: &str) -> IResult<&str, GraphPattern> {
    // First try to parse as a simple list of triples (most common case)
    if let Ok((remaining, patterns)) = parse_triple_patterns(input) {
        if !patterns.is_empty() {
            return Ok((remaining, GraphPattern::Basic(patterns)));
        }
    }

    // Otherwise parse as structured graph pattern elements
    let (input, patterns) = many1(terminated(parse_graph_pattern_element, ws))(input)?;

    if patterns.len() == 1 {
        Ok((input, patterns.into_iter().next().unwrap()))
    } else {
        Ok((input, GraphPattern::Group(patterns)))
    }
}

/// Parse a single graph pattern element.
fn parse_graph_pattern_element(input: &str) -> IResult<&str, GraphPattern> {
    alt((
        parse_optional_pattern,
        parse_union_pattern,
        parse_filter_pattern,
        parse_bind_pattern,
        parse_basic_pattern,
    ))(input)
}

/// Parse an OPTIONAL pattern.
fn parse_optional_pattern(input: &str) -> IResult<&str, GraphPattern> {
    let (input, _) = tag_no_case("OPTIONAL")(input)?;
    let (input, _) = ws(input)?;
    let (input, pattern) = parse_graph_pattern(input)?;

    Ok((input, GraphPattern::Optional(Box::new(pattern))))
}

/// Parse a UNION pattern.
fn parse_union_pattern(input: &str) -> IResult<&str, GraphPattern> {
    let (input, left) = parse_graph_pattern(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = tag_no_case("UNION")(input)?;
    let (input, _) = ws(input)?;
    let (input, right) = parse_graph_pattern(input)?;

    Ok((input, GraphPattern::Union(Box::new(left), Box::new(right))))
}

/// Parse a FILTER pattern.
fn parse_filter_pattern(input: &str) -> IResult<&str, GraphPattern> {
    let (input, _) = tag_no_case("FILTER")(input)?;
    let (input, _) = ws(input)?;
    let (input, expr) = delimited(
        char('('),
        preceded(ws, parse_filter_expr),
        preceded(ws, char(')')),
    )(input)?;
    let (input, _) = ws(input)?;
    let (input, pattern) = parse_graph_pattern(input)?;

    Ok((
        input,
        GraphPattern::Filter {
            expr,
            pattern: Box::new(pattern),
        },
    ))
}

/// Parse a BIND pattern.
fn parse_bind_pattern(input: &str) -> IResult<&str, GraphPattern> {
    let (input, _) = tag_no_case("BIND")(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('(')(input)?;
    let (input, _) = ws(input)?;
    let (input, expr) = parse_bind_expr(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = tag_no_case("AS")(input)?;
    let (input, _) = ws(input)?;
    let (input, var) = parse_var(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(')')(input)?;

    Ok((input, GraphPattern::Bind { expr, var }))
}

/// Parse a basic graph pattern (list of triples).
fn parse_basic_pattern(input: &str) -> IResult<&str, GraphPattern> {
    let (input, patterns) = parse_triple_patterns(input)?;
    Ok((input, GraphPattern::Basic(patterns)))
}

/// Parse multiple triple patterns separated by periods.
fn parse_triple_patterns(input: &str) -> IResult<&str, Vec<TriplePattern>> {
    let (input, first) = opt(parse_triple_pattern)(input)?;
    let mut patterns = Vec::new();

    if let Some(pattern) = first {
        patterns.push(pattern);
        let (mut remaining, _) = ws(input)?;

        // Parse additional patterns separated by periods
        while let Ok((after_period, _)) = char::<_, nom::error::Error<&str>>('.')(remaining) {
            let (after_ws, _) = ws(after_period)?;
            // Try to parse another pattern
            if let Ok((after_pattern, pattern)) = parse_triple_pattern(after_ws) {
                patterns.push(pattern);
                let (after_ws2, _) = ws(after_pattern)?;
                remaining = after_ws2;
            } else {
                // Period without following pattern - put it back
                break;
            }
        }

        Ok((remaining, patterns))
    } else {
        Ok((input, patterns))
    }
}

/// Parse a single triple pattern.
fn parse_triple_pattern(input: &str) -> IResult<&str, TriplePattern> {
    let (input, subject) = parse_term(input)?;
    let (input, _) = ws1(input)?;
    let (input, predicate) = parse_term(input)?;
    let (input, _) = ws1(input)?;
    let (input, object) = parse_term(input)?;
    let (input, _) = ws(input)?;

    Ok((
        input,
        TriplePattern {
            subject,
            predicate,
            object,
        },
    ))
}

/// Parse a term (variable, IRI, prefixed name, or literal).
fn parse_term(input: &str) -> IResult<&str, Term> {
    alt((
        map(parse_var, Term::Var),
        map(parse_iri_ref, |iri| Term::Iri(Iri { iri })),
        parse_prefixed_name_term,
        map(parse_literal, Term::Literal),
        parse_blank_node,
    ))(input)
}

/// Parse a variable (?name or $name).
fn parse_var(input: &str) -> IResult<&str, Var> {
    let (input, _) = one_of("?$")(input)?;
    let (input, name) = map(
        take_while1(|c: char| c.is_alphanumeric() || c == '_'),
        |s: &str| s.to_string(),
    )(input)?;

    Ok((input, Var { name }))
}

/// Parse a prefixed name (e.g., a2a:Entity).
fn parse_prefixed_name_term(input: &str) -> IResult<&str, Term> {
    let (input, prefix) = map(
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        |s: &str| s.to_string(),
    )(input)?;
    let (input, _) = char(':')(input)?;
    let (input, local) = map(
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        |s: &str| s.to_string(),
    )(input)?;

    Ok((input, Term::PrefixedName { prefix, local }))
}

/// Parse a literal.
fn parse_literal(input: &str) -> IResult<&str, Literal> {
    let (input, value) = alt((
        delimited(char('"'), take_until("\""), char('"')),
        delimited(tag("\"\"\""), take_until("\"\"\""), tag("\"\"\"")),
    ))(input)?;

    let (input, datatype) = opt(preceded(tag("^^"), map(parse_iri_ref, |iri| Iri { iri })))(input)?;

    let (input, language) = opt(preceded(
        char('@'),
        map(
            take_while1(|c: char| c.is_alphanumeric() || c == '-'),
            |s: &str| s.to_string(),
        ),
    ))(input)?;

    Ok((
        input,
        Literal {
            value: value.to_string(),
            datatype,
            language,
        },
    ))
}

/// Parse a blank node.
fn parse_blank_node(input: &str) -> IResult<&str, Term> {
    let (input, _) = tag("_:")(input)?;
    let (input, id) = map(
        take_while1(|c: char| c.is_alphanumeric() || c == '_'),
        |s: &str| s.to_string(),
    )(input)?;

    Ok((input, Term::BlankNode(id)))
}

/// Parse a filter expression.
fn parse_filter_expr(input: &str) -> IResult<&str, FilterExpr> {
    parse_logical_or_expr(input)
}

/// Parse logical OR expression.
fn parse_logical_or_expr(input: &str) -> IResult<&str, FilterExpr> {
    let (input, left) = parse_logical_and_expr(input)?;
    let (input, rights) = many0(preceded(
        delimited(ws, tag("||"), ws),
        parse_logical_and_expr,
    ))(input)?;

    Ok((
        input,
        rights
            .into_iter()
            .fold(left, |acc, right| FilterExpr::Logical {
                op: LogicalOp::Or,
                left: Box::new(acc),
                right: Box::new(right),
            }),
    ))
}

/// Parse logical AND expression.
fn parse_logical_and_expr(input: &str) -> IResult<&str, FilterExpr> {
    let (input, left) = parse_relational_expr(input)?;
    let (input, rights) = many0(preceded(
        delimited(ws, tag("&&"), ws),
        parse_relational_expr,
    ))(input)?;

    Ok((
        input,
        rights
            .into_iter()
            .fold(left, |acc, right| FilterExpr::Logical {
                op: LogicalOp::And,
                left: Box::new(acc),
                right: Box::new(right),
            }),
    ))
}

/// Parse relational expression.
fn parse_relational_expr(input: &str) -> IResult<&str, FilterExpr> {
    let (input, left) = parse_primary_expr(input)?;
    let (input, op_right) = opt(pair(
        delimited(
            ws,
            alt((
                value(RelationalOp::Eq, tag("=")),
                value(RelationalOp::Ne, tag("!=")),
                value(RelationalOp::Le, tag("<=")),
                value(RelationalOp::Ge, tag(">=")),
                value(RelationalOp::Lt, tag("<")),
                value(RelationalOp::Gt, tag(">")),
            )),
            ws,
        ),
        parse_primary_expr,
    ))(input)?;

    match op_right {
        Some((op, right)) => Ok((
            input,
            FilterExpr::Relational {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
        )),
        None => Ok((input, left)),
    }
}

/// Parse primary filter expression.
fn parse_primary_expr(input: &str) -> IResult<&str, FilterExpr> {
    alt((
        parse_bound_expr,
        parse_function_call,
        map(parse_var, FilterExpr::Var),
        map(parse_literal, FilterExpr::Literal),
        delimited(
            char('('),
            preceded(ws, parse_filter_expr),
            preceded(ws, char(')')),
        ),
    ))(input)
}

/// Parse BOUND(?var) expression.
fn parse_bound_expr(input: &str) -> IResult<&str, FilterExpr> {
    let (input, _) = tag_no_case("BOUND")(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('(')(input)?;
    let (input, _) = ws(input)?;
    let (input, var) = parse_var(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(')')(input)?;

    Ok((input, FilterExpr::Bound(var)))
}

/// Parse function call in filter.
fn parse_function_call(input: &str) -> IResult<&str, FilterExpr> {
    let (input, name) = map(
        take_while1(|c: char| c.is_alphanumeric() || c == '_'),
        |s: &str| s.to_string(),
    )(input)?;
    let (input, _) = ws(input)?;
    let (input, args) = delimited(
        char('('),
        preceded(
            ws,
            separated_list0(delimited(ws, char(','), ws), parse_filter_expr),
        ),
        preceded(ws, char(')')),
    )(input)?;

    Ok((input, FilterExpr::Function { name, args }))
}

/// Parse a BIND expression.
fn parse_bind_expr(input: &str) -> IResult<&str, BindExpr> {
    alt((
        parse_iri_function,
        parse_concat_function,
        parse_str_function,
        parse_if_function,
        map(parse_var, BindExpr::Var),
        map(parse_literal, BindExpr::Literal),
    ))(input)
}

/// Parse IRI() function.
fn parse_iri_function(input: &str) -> IResult<&str, BindExpr> {
    let (input, _) = tag_no_case("IRI")(input)?;
    let (input, _) = ws(input)?;
    let (input, expr) = delimited(
        char('('),
        preceded(ws, parse_bind_expr),
        preceded(ws, char(')')),
    )(input)?;

    Ok((input, BindExpr::Iri(Box::new(expr))))
}

/// Parse CONCAT() function.
fn parse_concat_function(input: &str) -> IResult<&str, BindExpr> {
    let (input, _) = tag_no_case("CONCAT")(input)?;
    let (input, _) = ws(input)?;
    let (input, exprs) = delimited(
        char('('),
        preceded(
            ws,
            separated_list1(delimited(ws, char(','), ws), parse_bind_expr),
        ),
        preceded(ws, char(')')),
    )(input)?;

    Ok((input, BindExpr::Concat(exprs)))
}

/// Parse STR() function.
fn parse_str_function(input: &str) -> IResult<&str, BindExpr> {
    let (input, _) = tag_no_case("STR")(input)?;
    let (input, _) = ws(input)?;
    let (input, expr) = delimited(
        char('('),
        preceded(ws, parse_bind_expr),
        preceded(ws, char(')')),
    )(input)?;

    Ok((input, BindExpr::Str(Box::new(expr))))
}

/// Parse IF() function.
fn parse_if_function(input: &str) -> IResult<&str, BindExpr> {
    let (input, _) = tag_no_case("IF")(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('(')(input)?;
    let (input, _) = ws(input)?;
    let (input, condition) = parse_filter_expr(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(',')(input)?;
    let (input, _) = ws(input)?;
    let (input, then_expr) = parse_bind_expr(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(',')(input)?;
    let (input, _) = ws(input)?;
    let (input, else_expr) = parse_bind_expr(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        BindExpr::If {
            condition,
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_var() {
        let input = "?name";
        let result = parse_var(input);
        assert!(result.is_ok());
        let (_, var) = result.unwrap();
        assert_eq!(var.name, "name");
    }

    #[test]
    fn test_parse_prefixed_name() {
        let input = "a2a:Entity";
        let result = parse_prefixed_name_term(input);
        assert!(result.is_ok());
        let (_, term) = result.unwrap();
        match term {
            Term::PrefixedName { prefix, local } => {
                assert_eq!(prefix, "a2a");
                assert_eq!(local, "Entity");
            }
            _ => panic!("Expected PrefixedName"),
        }
    }

    #[test]
    fn test_parse_simple_query() {
        let query = r#"
            PREFIX a2a: <https://ggen.io/ontology/a2a/>
            CONSTRUCT {
                ?s a2a:name ?name .
            }
            WHERE {
                ?s a2a:name ?name .
            }
        "#;

        let result = Parser::parse(query);
        if let Err(e) = &result {
            eprintln!("Parse error: {:?}", e);
        }
        assert!(result.is_ok(), "Failed to parse query: {:?}", result.err());
        let parsed = result.unwrap();
        assert_eq!(parsed.prefixes.len(), 1);
        assert_eq!(parsed.construct.patterns.len(), 1);
    }

    #[test]
    fn test_parse_optional() {
        let input = "OPTIONAL { ?s ?p ?o . }";
        let result = parse_optional_pattern(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_literal() {
        let input = r#""hello world""#;
        let result = parse_literal(input);
        assert!(result.is_ok());
        let (_, literal) = result.unwrap();
        assert_eq!(literal.value, "hello world");
    }
}
