use pest::Parser;
use pest::error::Error;
use pest::iterators::Pair;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct ToyParser;

// ---- AST --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Let {
        name: String,
        value: Expr,
    },
    Return(Expr),
    If {
        cond: Expr,
        then: Body,
        else_: Option<Box<Stmt>>,
    },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64),
    Var(String),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

// ---- Parsing ----------------------------------------------------------------

pub fn parse(input: &str) -> Result<Program, Error<Rule>> {
    let pairs = ToyParser::parse(Rule::program, input)?;
    let pair = pairs.into_iter().next().unwrap();
    Ok(build_program(pair))
}

fn build_program(pair: Pair<Rule>) -> Program {
    debug_assert!(pair.as_rule() == Rule::program);
    let functions = pair
        .into_inner()
        .filter(|p| p.as_rule() == Rule::function)
        .map(build_function)
        .collect();
    Program { functions }
}

fn build_function(pair: Pair<Rule>) -> Function {
    debug_assert!(pair.as_rule() == Rule::function);
    let mut inner = pair.into_inner();
    let name = ident_str(inner.next().unwrap());
    let next = inner.next().unwrap();
    let (params, block_pair) = match next.as_rule() {
        Rule::params => (
            next.into_inner().map(ident_str).collect(),
            inner.next().unwrap(),
        ),
        Rule::block => (Vec::new(), next),
        _ => unreachable!(),
    };
    let body = build_block(block_pair);
    Function { name, params, body }
}

fn build_block(pair: Pair<Rule>) -> Body {
    debug_assert!(pair.as_rule() == Rule::block);
    let stmts = pair
        .into_inner()
        .filter(|p| p.as_rule() == Rule::stmt)
        .map(build_stmt)
        .collect();
    Body { stmts }
}

fn build_stmt(pair: Pair<Rule>) -> Stmt {
    debug_assert!(pair.as_rule() == Rule::stmt);
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::let_stmt => {
            let mut it = inner.into_inner();
            let name = ident_str(it.next().unwrap());
            let value = build_expr(it.next().unwrap());
            Stmt::Let { name, value }
        }
        Rule::return_stmt => {
            let mut it = inner.into_inner();
            let value = build_expr(it.next().unwrap());
            Stmt::Return(value)
        }
        Rule::if_stmt => {
            let mut it = inner.into_inner();
            let cond = build_expr(it.next().unwrap());
            let then = build_block(it.next().unwrap());
            let else_ = it.next().map(|p| {
                // `p` is either an `if_stmt` or a `block` (the grammar wraps
                // the choice directly after `else`).
                match p.as_rule() {
                    Rule::if_stmt => Box::new(build_if_stmt(p)),
                    Rule::block => Box::new(Stmt::Expr(block_last_expr(build_block(p)))),
                    _ => unreachable!(),
                }
            });
            Stmt::If { cond, then, else_ }
        }
        Rule::expr_stmt => {
            let value = build_expr(inner.into_inner().next().unwrap());
            Stmt::Expr(value)
        }
        _ => unreachable!(),
    }
}

fn build_if_stmt(pair: Pair<Rule>) -> Stmt {
    debug_assert!(pair.as_rule() == Rule::if_stmt);
    let mut it = pair.into_inner();
    let cond = build_expr(it.next().unwrap());
    let then = build_block(it.next().unwrap());
    let else_ = it.next().map(|p| match p.as_rule() {
        Rule::if_stmt => Box::new(build_if_stmt(p)),
        Rule::block => Box::new(Stmt::Expr(block_last_expr(build_block(p)))),
        _ => unreachable!(),
    });
    Stmt::If { cond, then, else_ }
}

// For an `else { ... }` block we collapse it into a single statement. A real
// implementation would add a `Block` variant to `Stmt`; for this minimal
// language we only support single-statement else blocks, which is enough for
// `else { return ...; }` style code.
fn block_last_expr(block: Body) -> Expr {
    match block.stmts.into_iter().last() {
        Some(Stmt::Return(e)) | Some(Stmt::Expr(e)) => e,
        _ => Expr::Int(0),
    }
}

fn build_expr(pair: Pair<Rule>) -> Expr {
    debug_assert!(pair.as_rule() == Rule::expr);
    build_comparison(pair.into_inner().next().unwrap())
}

fn build_comparison(pair: Pair<Rule>) -> Expr {
    debug_assert!(pair.as_rule() == Rule::comparison);
    let mut it = pair.into_inner();
    let mut lhs = build_additive(it.next().unwrap());
    while let Some(op_pair) = it.next() {
        let op = match op_pair.as_str() {
            "==" => BinaryOp::Eq,
            "!=" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            "<=" => BinaryOp::Le,
            ">" => BinaryOp::Gt,
            ">=" => BinaryOp::Ge,
            _ => unreachable!(),
        };
        let rhs = build_additive(it.next().unwrap());
        lhs = Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
    lhs
}

fn build_additive(pair: Pair<Rule>) -> Expr {
    debug_assert!(pair.as_rule() == Rule::additive);
    let mut it = pair.into_inner();
    let mut lhs = build_multiplicative(it.next().unwrap());
    while let Some(op_pair) = it.next() {
        let op = match op_pair.as_str() {
            "+" => BinaryOp::Add,
            "-" => BinaryOp::Sub,
            _ => unreachable!(),
        };
        let rhs = build_multiplicative(it.next().unwrap());
        lhs = Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
    lhs
}

fn build_multiplicative(pair: Pair<Rule>) -> Expr {
    debug_assert!(pair.as_rule() == Rule::multiplicative);
    let mut it = pair.into_inner();
    let mut lhs = build_unary(it.next().unwrap());
    while let Some(op_pair) = it.next() {
        let op = match op_pair.as_str() {
            "*" => BinaryOp::Mul,
            "/" => BinaryOp::Div,
            _ => unreachable!(),
        };
        let rhs = build_unary(it.next().unwrap());
        lhs = Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
    lhs
}

fn build_unary(pair: Pair<Rule>) -> Expr {
    debug_assert!(pair.as_rule() == Rule::unary);
    let mut it = pair.into_inner();
    let first = it.next().unwrap();
    match first.as_rule() {
        Rule::unary_op => {
            let op = match first.as_str() {
                "-" => UnaryOp::Neg,
                "!" => UnaryOp::Not,
                _ => unreachable!(),
            };
            let expr = Box::new(build_unary(it.next().unwrap()));
            Expr::Unary { op, expr }
        }
        _ => build_primary(first),
    }
}

fn build_primary(pair: Pair<Rule>) -> Expr {
    debug_assert!(pair.as_rule() == Rule::primary);
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::integer => Expr::Int(inner.as_str().parse().unwrap()),
        Rule::ident => Expr::Var(ident_str(inner)),
        Rule::call => {
            let mut it = inner.into_inner();
            let name = ident_str(it.next().unwrap());
            let args = match it.next() {
                Some(args_pair) if args_pair.as_rule() == Rule::args => {
                    args_pair.into_inner().map(build_expr).collect()
                }
                _ => Vec::new(),
            };
            Expr::Call { name, args }
        }
        Rule::expr => build_expr(inner),
        _ => unreachable!(),
    }
}

fn ident_str(pair: Pair<Rule>) -> String {
    debug_assert!(pair.as_rule() == Rule::ident);
    pair.as_str().to_string()
}

// ---- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_function() {
        let src = "fn add(x, y) { return x + y; }";
        let prog = parse(src).unwrap();
        assert_eq!(prog.functions.len(), 1);
        let f = &prog.functions[0];
        assert_eq!(f.name, "add");
        assert_eq!(f.params, vec!["x".to_string(), "y".to_string()]);
        match &f.body.stmts[0] {
            Stmt::Return(Expr::Binary {
                op: BinaryOp::Add, ..
            }) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parse_let_and_call() {
        let src = "fn main() { let x = 42; foo(x); }";
        let prog = parse(src).unwrap();
        let f = &prog.functions[0];
        assert_eq!(f.body.stmts.len(), 2);
        assert!(matches!(&f.body.stmts[0], Stmt::Let { name, .. } if name == "x"));
        assert!(matches!(&f.body.stmts[1], Stmt::Expr(Expr::Call { name, .. }) if name == "foo"));
    }

    #[test]
    fn parse_if_else() {
        let src = "fn f(n) { if n { return 1; } else { return 2; } }";
        parse(src).unwrap();
    }
}
