use anyhow::{ensure, Error, Result as AnyResult};
use nom::{
    branch::alt,
    bytes::complete::tag as nom_tag,
    character::complete::{
        alpha1 as nom_alpha1, alphanumeric1 as nom_alphanumeric1, digit1, multispace0,
    },
    combinator::{map, opt, recognize},
    error::{Error as NomError, ErrorKind, ParseError},
    multi::{fold_many1, many0, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    Err, Parser,
};
use nom_locate::LocatedSpan;

use crate::{
    ast::{Binding, ExprKind},
    context::ParsingContext,
};

pub(crate) fn parse_input(input_code: &str) -> (AnyResult<ExprKind>, ParsingContext) {
    let parsing_ctxt = ParsingContext::new();
    let input = LocatedSpan::new_extra(input_code, &parsing_ctxt);
    let parsing_status = program(input)
        .map_err(own_nom_err)
        .map_err(Error::new)
        .and_then(|(tail, expr)| {
            ensure!(
                tail.is_empty(),
                "Parser did not consume the whole program: {} remains",
                tail
            );
            Ok(expr)
        });

    (parsing_status, parsing_ctxt)
}

fn own_nom_err(err: Err<nom::error::Error<Input>>) -> Err<nom::error::Error<()>> {
    match err {
        Err::Error(e) => Err::Error(own_nom_error(e)),
        Err::Failure(f) => Err::Failure(own_nom_error(f)),
        Err::Incomplete(needed) => Err::Incomplete(needed),
    }
}

fn own_nom_error(err: NomError<Input>) -> NomError<()> {
    let NomError { code, .. } = err;
    let input = ();
    NomError { input, code }
}

type Input<'a> = LocatedSpan<&'a str, &'a ParsingContext>;
type IResult<'a, O, E = NomError<Input<'a>>> = nom::IResult<Input<'a>, O, E>;

fn program(input: Input) -> IResult<ExprKind> {
    alt((bindings, expr))(input)
}

fn block(input: Input) -> IResult<ExprKind> {
    delimited(left_curly, alt((bindings, expr)), right_curly)(input)
}

fn expr(input: Input) -> IResult<ExprKind> {
    alt((level_0_expression, level_1_expression, atomic_expr))(input)
}

fn integer(input: Input) -> IResult<ExprKind> {
    let maybe_minus = opt(tag("-"));

    map(
        space_insignificant(recognize(tuple((maybe_minus, digit1)))),
        |i| ExprKind::integer(i.fragment().parse().unwrap()),
    )(input)
}

fn level_0_expression(input: Input) -> IResult<ExprKind> {
    let (tail, first) = alt((level_1_expression, atomic_expr))(input)?;

    fold_many1(
        tuple((level_0_operator, alt((level_1_expression, atomic_expr)))),
        first,
        |left, (operator, right)| operator.make_expr(left, right),
    )(tail)
}

fn level_0_operator(input: Input) -> IResult<Level0Operator> {
    map(alt((tag("+"), tag("-"))), |operator| match operator {
        "+" => Level0Operator::Plus,
        "-" => Level0Operator::Minus,
        _ => unreachable!(),
    })(input)
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Level0Operator {
    Plus,
    Minus,
}

impl Level0Operator {
    fn make_expr(self, lhs: ExprKind, rhs: ExprKind) -> ExprKind {
        let expression_maker = match self {
            Level0Operator::Plus => ExprKind::addition,
            Level0Operator::Minus => ExprKind::subtraction,
        };

        expression_maker(lhs, rhs)
    }
}

fn level_1_expression(input: Input) -> IResult<ExprKind> {
    let (tail, first) = atomic_expr(input)?;
    fold_many1(tuple((star, atomic_expr)), first, |lhs, (_, rhs)| {
        ExprKind::multiplication(lhs, rhs)
    })(tail)
}

fn star(input: Input) -> IResult<()> {
    map(space_insignificant(tag("*")), drop)(input)
}

fn if_else(input: Input) -> IResult<ExprKind> {
    let (tail, _) = if_(input)?;
    let (tail, condition) = expr(tail)?;
    let (tail, consequent) = block(tail)?;
    let (tail, _) = else_(tail)?;
    let (tail, alternative) = block(tail)?;

    let if_ = ExprKind::if_(condition, consequent, alternative);
    Ok((tail, if_))
}

fn bindings(input: Input) -> IResult<ExprKind> {
    let (tail, bs) = many1(binding)(input)?;
    let (tail, ending) = expr(tail)?;
    let bindings = ExprKind::bindings(bs, ending);

    Ok((tail, bindings))
}

fn binding(input: Input) -> IResult<Binding> {
    let (tail, name) = delimited(let_, ident, expect(equal, epsilon_recovery))(input)?;
    let (tail, value) = terminated(expr, expect(semicolon, epsilon_recovery))(tail)?;
    Ok((tail, Binding::new(name, value)))
}

fn atomic_expr(input: Input) -> IResult<ExprKind> {
    alt((integer, if_else, block, ident_expr))(input)
}

fn ident_expr(input: Input) -> IResult<ExprKind> {
    let (tail, name) = ident(input)?;
    Ok((tail, ExprKind::ident(name)))
}

fn ident(input: Input) -> IResult<String> {
    let (tail, name) = space_insignificant(recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    )))(input)?;

    Ok((tail, name.to_string()))
}

fn if_(input: Input) -> IResult<()> {
    keyword("if")(input)
}

fn else_(input: Input) -> IResult<()> {
    keyword("else")(input)
}

fn let_(input: Input) -> IResult<()> {
    keyword("let")(input)
}

fn equal(input: Input) -> IResult<()> {
    map(space_insignificant(tag("=")), drop)(input)
}

fn semicolon(input: Input) -> IResult<()> {
    map(space_insignificant(tag(";")), drop)(input)
}

fn keyword(kw: &str) -> impl Fn(Input) -> IResult<()> + '_ {
    move |input| {
        let (tail, _) = map(preceded(multispace0, tag(kw)), drop)(input)?;
        let next_is_alphabetic = tail
            .chars()
            .next()
            .map(char::is_alphabetic)
            .unwrap_or(false);

        if next_is_alphabetic {
            Err(Err::Error(NomError::new(input, ErrorKind::Tag)))
        } else {
            let (tail, _) = multispace0(tail)?;
            Ok((tail, ()))
        }
    }
}

fn left_curly(input: Input) -> IResult<()> {
    map(space_insignificant(tag("{")), drop)(input)
}

fn right_curly(input: Input) -> IResult<()> {
    map(space_insignificant(tag("}")), drop)(input)
}

fn space_insignificant<'a, O, E>(
    parser: impl Parser<Input<'a>, O, E>,
) -> impl FnMut(Input<'a>) -> IResult<'a, O, E>
where
    E: ParseError<Input<'a>>,
{
    delimited(multispace0, parser, multispace0)
}

fn expect<O, P, R>(mut parser: P, mut recovery: R) -> impl FnMut(Input) -> IResult<Option<O>>
where
    P: FnMut(Input) -> IResult<O>,
    R: FnMut(Input, ErrorKind) -> Option<Input>,
{
    move |input| match parser(input) {
        Ok((tail, data)) => Ok((tail, Some(data))),
        Err(Err::Incomplete(_)) => panic!("Parser returned Incomplete variant"),
        Err(Err::Error(NomError { input, code })) | Err(Err::Failure(NomError { input, code })) => {
            match recovery(input, code) {
                Some(tail) => Ok((tail, None)),
                None => Ok((input, None)),
            }
        }
    }
}

fn epsilon_recovery(input: Input, _: ErrorKind) -> Option<Input> {
    input.extra.errors().add("Excepted token");
    Some(input)
}

fn tag<'a>(t: &'a str) -> impl FnMut(Input) -> IResult<&str> + 'a {
    move |input: Input| {
        map(nom_tag(t), |matched: LocatedSpan<&str, _>| {
            *matched.fragment()
        })(input)
    }
}

fn alphanumeric1(input: Input) -> IResult<&str> {
    map(nom_alphanumeric1, |matched: LocatedSpan<&str, _>| {
        *matched.fragment()
    })(input)
}

fn alpha1(input: Input) -> IResult<&str> {
    map(nom_alpha1, |matched: LocatedSpan<&str, _>| {
        *matched.fragment()
    })(input)
}

#[cfg(test)]
fn parse_and_own<O>(
    f: impl Fn(Input) -> IResult<O>,
    input: &str,
) -> (Result<O, Err<NomError<()>>>, ParsingContext) {
    let parsing_ctxt = ParsingContext::new();
    let input = LocatedSpan::new_extra(input, &parsing_ctxt);
    let parsing_status = f(input).map_err(own_nom_err).map(|(_, parsed)| parsed);

    (parsing_status, parsing_ctxt)
}

#[cfg(test)]
macro_rules! parse {
    ($rule:ident $slice:expr) => {{
        parse_and_own($rule, $slice)
    }};
}

#[cfg(test)]
mod program {
    use super::*;

    // TODO: once we get function parsing, replace it with a set of functions

    #[test]
    fn handles_bindings() {
        let (left, _) = parse! { program "let a = 40; let b = 2; a + b" };
        let right = Ok(ExprKind::bindings(
            vec![
                Binding::new("a".to_owned(), ExprKind::integer(40)),
                Binding::new("b".to_owned(), ExprKind::integer(2)),
            ],
            ExprKind::addition(
                ExprKind::ident("a".to_owned()),
                ExprKind::ident("b".to_owned()),
            ),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn handles_expression() {
        let (left, _) = parse! { program "1 + 2 + 2" };
        let right = Ok(ExprKind::addition(
            ExprKind::addition(ExprKind::integer(1), ExprKind::integer(2)),
            ExprKind::integer(2),
        ));

        assert_eq!(left, right);
    }
}

#[cfg(test)]
mod block {
    use super::*;

    #[test]
    fn handles_bindings() {
        let (left, _) = parse! { block "{ let a = 42; a }" };
        let right = Ok(ExprKind::bindings(
            vec![Binding::new("a".to_owned(), ExprKind::integer(42))],
            ExprKind::ident("a".to_owned()),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn handles_expression() {
        let (left, _) = parse! { block "{ 42 }" };
        let right = Ok(ExprKind::integer(42));

        assert_eq!(left, right);
    }
}

#[cfg(test)]
mod expr {
    use super::*;

    #[test]
    fn if_addition_parses() {
        let (left, _) = parse! { expr "if 1 { 1 } else { 1 } + 1" };
        let right = Ok(ExprKind::addition(
            ExprKind::if_(
                ExprKind::integer(1),
                ExprKind::integer(1),
                ExprKind::integer(1),
            ),
            ExprKind::integer(1),
        ));

        assert_eq!(left, right);
    }
}
#[cfg(test)]
mod integer {
    use super::*;

    #[test]
    fn integer_simple() {
        let (left, _) = parse! { integer "42" };
        let right = Ok(ExprKind::integer(42));

        assert_eq!(left, right);
    }

    #[test]
    fn integer_with_tail() {
        let ctxt = ParsingContext::new();
        let file = LocatedSpan::new_extra("101 !", &ctxt);

        let left = integer(file).map(|(tail, parsed)| (*tail.fragment(), parsed));
        let right = Ok(("!", ExprKind::integer(101)));

        assert_eq!(left, right);
    }

    #[test]
    fn integer_failing_when_not_digit() {
        assert!(parse! { integer "abc" }.0.is_err());
        assert!(parse! { integer "" }.0.is_err());
    }

    #[test]
    fn integer_eats_whitespaces_before_and_after() {
        let (left, _) = parse! { integer " 42 " };
        let right = Ok(ExprKind::integer(42));

        assert_eq!(left, right);
    }

    #[test]
    fn negative() {
        let (left, _) = parse! { integer "-101" };
        let right = Ok(ExprKind::integer(-101));

        assert_eq!(left, right);
    }
}

#[cfg(test)]
mod add_and_sub {
    use super::*;

    #[test]
    fn single_factor_fails() {
        assert!(parse! { level_0_expression "42" }.0.is_err());
    }

    #[test]
    fn addition_simple() {
        let (left, _) = parse! { level_0_expression "1+1" };
        let right = Ok(ExprKind::addition(
            ExprKind::integer(1),
            ExprKind::integer(1),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn addition_right_associative() {
        let (left, _) = parse! { level_0_expression "1+1+1" };
        let right = Ok(ExprKind::addition(
            ExprKind::addition(ExprKind::integer(1), ExprKind::integer(1)),
            ExprKind::integer(1),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn subtraction_simple() {
        let (left, _) = parse! { level_0_expression "43-1" };
        let right = Ok(ExprKind::subtraction(
            ExprKind::integer(43),
            ExprKind::integer(1),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn subtraction_right_associative() {
        let (left, _) = parse! { level_0_expression "44-1-1" };
        let right = Ok(ExprKind::subtraction(
            ExprKind::subtraction(ExprKind::integer(44), ExprKind::integer(1)),
            ExprKind::integer(1),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn addition_subtraction_mixed() {
        let (left, _) = parse! { level_0_expression "42-1+1" };
        let right = Ok(ExprKind::addition(
            ExprKind::subtraction(ExprKind::integer(42), ExprKind::integer(1)),
            ExprKind::integer(1),
        ));

        assert_eq!(left, right);
    }
}

#[cfg(test)]
mod mul {
    use super::*;

    #[test]
    fn parse_simple() {
        let (left, _) = parse! { level_1_expression "7*6" };
        let right = Ok(ExprKind::multiplication(
            ExprKind::integer(7),
            ExprKind::integer(6),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn when_spaced() {
        let (left, _) = parse! { level_1_expression "21 * 2" };
        let right = Ok(ExprKind::multiplication(
            ExprKind::integer(21),
            ExprKind::integer(2),
        ));

        assert_eq!(left, right);
    }
}

#[cfg(test)]
mod math {
    use super::*;

    #[test]
    fn priority_simple() {
        let (left, _) = parse! { level_0_expression "10 * 4 + 2" };
        let right = Ok(ExprKind::addition(
            ExprKind::multiplication(ExprKind::integer(10), ExprKind::integer(4)),
            ExprKind::integer(2),
        ));

        assert_eq!(left, right);
    }
}

#[cfg(test)]
mod if_else {
    use super::*;

    #[test]
    fn if_else_simple() {
        let (left, _) = parse! { if_else "if0{1}else{42}" };
        let right = Ok(ExprKind::if_(
            ExprKind::integer(0),
            ExprKind::integer(1),
            ExprKind::integer(42),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn if_else_spaced_braces() {
        let (left, _) = parse! { if_else "if 0 { 1 } else { 42 }" };
        let right = Ok(ExprKind::if_(
            ExprKind::integer(0),
            ExprKind::integer(1),
            ExprKind::integer(42),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn addition_as_condition() {
        let (left, _) = parse! { if_else "if 1 + 1 { 1 } else { 1 }" };
        let right = Ok(ExprKind::if_(
            ExprKind::addition(ExprKind::integer(1), ExprKind::integer(1)),
            ExprKind::integer(1),
            ExprKind::integer(1),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn bindings_as_consequent_and_alternative() {
        let (left, _) = parse! { if_else "if 1 { let a = 0; a } else { let a = 0; a }" };
        let inner_bindings = ExprKind::bindings(
            vec![Binding::new("a".to_owned(), ExprKind::integer(0))],
            ExprKind::ident("a".to_owned()),
        );
        let right = Ok(ExprKind::if_(
            ExprKind::integer(1),
            inner_bindings.clone(),
            inner_bindings,
        ));

        assert_eq!(left, right);
    }
}

#[cfg(test)]
mod keyword {
    use super::*;

    #[test]
    fn parses() {
        let if_ = keyword("if");
        let (left, _) = parse! { if_ "if" };
        let right = Ok(());

        assert_eq!(left, right);
    }

    #[test]
    fn fails_when_followed_by_letter() {
        let if_ = keyword("if");
        assert!(parse! { if_ "iff" }.0.is_err());
    }

    #[test]
    fn works_when_followed_by_non_letter() {
        let if_ = keyword("if");
        let (left, _) = parse! { if_ "if42" };
        let right = Ok(());

        assert_eq!(left, right);
    }

    #[test]
    fn kw_followed_by_space_and_letter() {
        let let_ = keyword("let");
        assert!(parse! { let_ "let a" }.0.is_ok());
    }
}

#[cfg(test)]
mod binding {
    use super::*;

    #[test]
    fn simple() {
        let (left, _) = parse! { binding "let a = 42;" };
        let right = Ok(Binding::new("a".to_owned(), ExprKind::integer(42)));

        assert_eq!(left, right);
    }

    #[test]
    fn with_if_else() {
        let (left, _) = parse! { binding "let foo = if 5 { 42 } else { 101 };" };
        let right = Ok(Binding::new(
            "foo".to_owned(),
            ExprKind::if_(
                ExprKind::integer(5),
                ExprKind::integer(42),
                ExprKind::integer(101),
            ),
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn recovers_on_missing_equal() {
        assert!(parse! { binding "let x 42;" }.0.is_ok());
    }

    #[test]
    fn recovers_on_missing_semicolon() {
        assert!(parse! { binding "let x = 42" }.0.is_ok());
    }
}

#[cfg(test)]
mod bindings {
    use super::*;

    #[test]
    fn bindings_simple() {
        let (left, _) = parse! { bindings "let a = 42; a" };
        let right = Ok(ExprKind::single_binding(
            "a".to_owned(),
            ExprKind::integer(42),
            ExprKind::ident("a".to_owned()),
        ));

        assert_eq!(left, right);
    }
}
