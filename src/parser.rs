use crate::ast::{Ast, MatchClause, Pattern};
use crate::diagnostics::SelError;
use crate::lexer::{Lexer, Loc, NumberBase, Token, TokenKind};
use crate::types::{intern, lookup};

type Result<T> = std::result::Result<T, SelError>;

static PARSER_GEN_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn gen_parser_id(prefix: &str) -> u32 {
    let id = PARSER_GEN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    intern(&format!("{}_{}", prefix, id))
}

fn is_sym(ast: &Ast, target: &str) -> bool {
    matches!(ast, Ast::Symbol(_, id) if lookup(*id) == target)
}

pub fn optimize_ast(list: Vec<Ast>, loc: Loc) -> Result<Ast> {
    if list.is_empty() {
        return Ok(Ast::Nil(loc));
    }

    if let Some(Ast::Symbol(s_loc, id)) = list.first().cloned() {
        match lookup(id).as_str() {
            "co-yield" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().unwrap_or(Ast::Nil(s_loc));
                Ok(Ast::Yield(s_loc, Box::new(expr)))
            }
            "co-resume" => {
                let mut iter = list.into_iter().skip(1);
                let co = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing coroutine in co-resume".into())
                })?;
                let arg = iter.next().unwrap_or(Ast::Nil(s_loc));
                Ok(Ast::CoResume(s_loc, Box::new(co), Box::new(arg)))
            }
            "try" => parse_try(list, s_loc),
            "->" => {
                let mut iter = list.into_iter().skip(1);
                let mut first_v = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing first expression in `->`".into())
                })?;
                for ast in iter {
                    match ast {
                        Ast::List(f_loc, mut f_list) => {
                            if f_list.is_empty() {
                                f_list.push(first_v);
                            } else {
                                f_list.insert(1, first_v);
                            }
                            first_v = Ast::List(f_loc, f_list);
                        }
                        s => {
                            let f_loc = s.loc();
                            first_v = Ast::List(f_loc, vec![s, first_v]);
                        }
                    }
                }
                Ok(first_v)
            }
            "|>" => {
                let mut iter = list.into_iter().skip(1);
                let mut first_v = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing first expression in `|>`".into())
                })?;
                for ast in iter {
                    match ast {
                        Ast::List(f_loc, mut f_list) => {
                            f_list.push(first_v);
                            first_v = Ast::List(f_loc, f_list);
                        }
                        s => {
                            let f_loc = s.loc();
                            first_v = Ast::List(f_loc, vec![s, first_v]);
                        }
                    }
                }
                Ok(first_v)
            }
            "defn" => parse_defn(list, s_loc),
            "match-lambda" => {
                let iter = list.into_iter().skip(1);
                let mut clauses = Vec::new();
                for clause_ast in iter {
                    clauses.push(parse_match_clause(clause_ast)?);
                }
                if clauses.is_empty() {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected at least one clause in match-lambda".into(),
                    ));
                }
                let arg_sym = gen_parser_id("_match_arg");
                Ok(Ast::Lambda(
                    s_loc,
                    vec![arg_sym],
                    vec![Ast::Match(
                        s_loc,
                        Box::new(Ast::Symbol(s_loc, arg_sym)),
                        clauses,
                    )],
                ))
            }
            "with" => parse_with(list, s_loc),
            "for" => parse_for(list, s_loc),
            "delay" => {
                let body: Vec<Ast> = list.into_iter().skip(1).collect();
                if body.is_empty() {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected expression in delay".into(),
                    ));
                }
                Ok(Ast::Lambda(s_loc, Vec::new(), body))
            }
            "assert" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected condition in assert".into())
                })?;
                let mut err_call = vec![Ast::Symbol(s_loc, intern("error"))];
                if let Some(msg) = iter.next() {
                    err_call.push(Ast::String(s_loc, "Assert failed:".to_string()));
                    err_call.push(msg);
                    for extra in iter {
                        err_call.push(extra);
                    }
                } else {
                    err_call.push(Ast::String(s_loc, "Assert failed".to_string()));
                }
                Ok(Ast::Unless(
                    s_loc,
                    Box::new(cond),
                    Box::new(Ast::List(s_loc, err_call)),
                    None,
                ))
            }
            "import" => {
                let mut iter = list.into_iter().skip(1);
                let first = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected module name in import".into())
                })?;

                let (symbol, mut iter) = match first {
                    Ast::Symbol(_, symbol) => (symbol, iter),
                    Ast::String(_, s) => (intern(&s), iter),
                    Ast::List(_, inner_list) => {
                        // e.g. (import (foo :as f)) or (import ("foo" :as f)) or (import (foo f))
                        if inner_list.is_empty() {
                            return Err(SelError::SyntaxError(s_loc, "Empty import list".into()));
                        }
                        let mut inner_iter = inner_list.into_iter();
                        let first_inner = inner_iter.next().unwrap();
                        let symbol = match first_inner {
                            Ast::Symbol(_, symbol) => symbol,
                            Ast::String(_, s) => intern(&s),
                            _ => {
                                return Err(SelError::SyntaxError(
                                    s_loc,
                                    "Expected module name as symbol or string in import list"
                                        .into(),
                                ));
                            }
                        };
                        if let Some(next) = inner_iter.next() {
                            match next {
                                Ast::Symbol(_, as_sym) if lookup(as_sym) == ":as" => {
                                    let alias_ast = inner_iter.next().ok_or_else(|| {
                                        SelError::SyntaxError(
                                            s_loc,
                                            "Expected alias after :as".into(),
                                        )
                                    })?;
                                    if let Ast::Symbol(_, alias) = alias_ast {
                                        return Ok(Ast::Import(s_loc, symbol, Some(alias)));
                                    } else {
                                        return Err(SelError::SyntaxError(
                                            s_loc,
                                            "Expected symbol for alias".into(),
                                        ));
                                    }
                                }
                                Ast::Symbol(_, alias) => {
                                    // e.g. (import (foo f))
                                    return Ok(Ast::Import(s_loc, symbol, Some(alias)));
                                }
                                _ => {
                                    return Err(SelError::SyntaxError(
                                        s_loc,
                                        "Expected alias or :as keyword".into(),
                                    ));
                                }
                            }
                        } else {
                            return Ok(Ast::Import(s_loc, symbol, None));
                        }
                    }
                    _ => {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Expected symbol, string, or list in import".into(),
                        ));
                    }
                };

                // Check if there's an inline :as alias, e.g. (import foo :as f) or (import "foo" :as f)
                if let Some(next) = iter.next() {
                    if let Ast::Symbol(_, as_sym) = next {
                        if lookup(as_sym) == ":as" {
                            let alias_ast = iter.next().ok_or_else(|| {
                                SelError::SyntaxError(s_loc, "Expected alias after :as".into())
                            })?;
                            if let Ast::Symbol(_, alias) = alias_ast {
                                Ok(Ast::Import(s_loc, symbol, Some(alias)))
                            } else {
                                Err(SelError::SyntaxError(
                                    s_loc,
                                    "Expected symbol for alias".into(),
                                ))
                            }
                        } else {
                            Err(SelError::SyntaxError(
                                s_loc,
                                "Expected :as keyword for alias".into(),
                            ))
                        }
                    } else {
                        Err(SelError::SyntaxError(
                            s_loc,
                            "Expected symbol for alias keyword".into(),
                        ))
                    }
                } else {
                    Ok(Ast::Import(s_loc, symbol, None))
                }
            }
            "load" => {
                let mut iter = list.into_iter().skip(1);
                let path = iter
                    .next()
                    .ok_or_else(|| SelError::SyntaxError(s_loc, "Missing path in load".into()))?;
                if iter.next().is_some() {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected exactly 1 argument for load".into(),
                    ));
                }
                Ok(Ast::Load(s_loc, Box::new(path)))
            }
            "while" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in while".into())
                })?;
                let body: Vec<Ast> = iter.collect();
                Ok(Ast::While(
                    s_loc,
                    Box::new(cond),
                    Box::new(Ast::List(loc, body)),
                ))
            }
            "until" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in until".into())
                })?;
                let body: Vec<Ast> = iter.collect();
                Ok(Ast::Until(
                    s_loc,
                    Box::new(cond),
                    Box::new(Ast::List(loc, body)),
                ))
            }
            "if" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in if".into())
                })?;
                let true_branch = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing true branch in if".into())
                })?;
                let false_branch = iter.next();
                Ok(Ast::If(
                    s_loc,
                    Box::new(cond),
                    Box::new(true_branch),
                    false_branch.map(Box::new),
                ))
            }
            "cond" => {
                let iter = list.into_iter().skip(1);
                let mut branches = Vec::new();
                for clause in iter {
                    match clause {
                        Ast::List(c_loc, mut items) => {
                            if items.is_empty() {
                                return Err(SelError::SyntaxError(
                                    c_loc,
                                    "Empty clause in cond".into(),
                                ));
                            }
                            let raw_cond = items.remove(0);
                            let cond_ast = if let Ast::Symbol(_, sym_id) = &raw_cond
                                && matches!(lookup(*sym_id).as_str(), "else" | ":else")
                            {
                                Ast::Boolean(c_loc, true)
                            } else {
                                raw_cond
                            };
                            if !items.is_empty() && is_sym(&items[0], ":do") {
                                items.remove(0);
                            }
                            let body_ast = if items.is_empty() {
                                Ast::Nil(c_loc)
                            } else if items.len() == 1 {
                                items.remove(0)
                            } else {
                                Ast::Begin(c_loc, items)
                            };
                            branches.push((cond_ast, body_ast));
                        }
                        other => {
                            return Err(SelError::SyntaxError(
                                other.loc(),
                                "Expected list clause (test body...) in cond".into(),
                            ));
                        }
                    }
                }
                Ok(Ast::Cond(s_loc, branches))
            }
            "ffi-func" => {
                // (define puts (ffi-func 'i32 '('*u8)))
                let mut iter = list.into_iter().skip(1);
                let sym = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing ffi-sym in ffi-func".into())
                })?;
                let ret = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing return type in ffi-func".into())
                })?;
                let arg_types = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing argument types in ffi-func".into())
                })?;
                // ffi-call ~sym ~ret ~arg-types args
                Ok(Ast::Lambda(
                    s_loc,
                    vec![intern("&args")],
                    vec![Ast::List(
                        s_loc,
                        vec![
                            Ast::Symbol(s_loc, intern("ffi-call")),
                            sym,
                            ret,
                            arg_types,
                            Ast::Symbol(s_loc, intern("args")),
                        ],
                    )],
                ))
            }
            "unless" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in unless".into())
                })?;
                let false_branch = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing false branch in unless".into())
                })?;
                let true_branch = iter.next();
                Ok(Ast::Unless(
                    s_loc,
                    Box::new(cond),
                    Box::new(false_branch),
                    true_branch.map(Box::new),
                ))
            }
            "when" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in when".into())
                })?;
                let body = iter.collect();
                Ok(Ast::When(s_loc, Box::new(cond), body))
            }
            "lambda" | "fn" => {
                let mut iter = list.into_iter().skip(1);
                let params_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing parameters in lambda".into())
                })?;
                let mut params = Vec::new();
                match params_ast {
                    Ast::List(loc, p) => {
                        for param in p {
                            if let Ast::Symbol(_, id) = param {
                                params.push(id);
                            } else if let Ast::Bind(_, id) = param {
                                let name = lookup(id);
                                params.push(intern(&format!("&{}", name)));
                            } else {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected identifier in lambda parameters".into(),
                                ));
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Expected parameter list in lambda".into(),
                        ));
                    }
                }
                let body = iter.collect();
                Ok(Ast::Lambda(s_loc, params, body))
            }
            "begin" => {
                let iter = list.into_iter().skip(1);
                Ok(Ast::Begin(s_loc, iter.collect()))
            }
            "define" => {
                let mut iter = list.into_iter().skip(1);
                let name_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected identifier in define".into())
                })?;
                if let Ast::List(l_loc, mut p_list) = name_ast {
                    if p_list.is_empty() {
                        return Err(SelError::SyntaxError(
                            l_loc,
                            "Empty parameter list in define".into(),
                        ));
                    }
                    let head = p_list.remove(0);
                    let Ast::Symbol(_, name_id) = head else {
                        return Err(SelError::SyntaxError(
                            l_loc,
                            "Expected identifier at head of parameter list in define".into(),
                        ));
                    };
                    let mut params = Vec::new();
                    for p in p_list {
                        match p {
                            Ast::Symbol(_, id) => params.push(id),
                            Ast::Bind(_, id) => {
                                let name = lookup(id);
                                params.push(intern(&format!("&{}", name)));
                            }
                            _ => {
                                return Err(SelError::SyntaxError(
                                    l_loc,
                                    "Expected identifier in parameter list".into(),
                                ));
                            }
                        }
                    }
                    let body: Vec<Ast> = iter.collect();
                    if body.is_empty() {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Missing body in define".into(),
                        ));
                    }
                    return Ok(Ast::Define(
                        s_loc,
                        name_id,
                        Box::new(Ast::Lambda(s_loc, params, body)),
                    ));
                }

                let value_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected expression in define".into())
                })?;
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected identifier in define".into(),
                    ));
                };
                Ok(Ast::Define(s_loc, name_id, Box::new(value_ast)))
            }
            "defmacro" => {
                let mut iter = list.into_iter().skip(1);
                let name_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected identifier in defmacro".into())
                })?;
                let params_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected parameters in defmacro".into())
                })?;

                let mut params = Vec::new();
                match params_ast {
                    Ast::List(_, p) => {
                        for param in p {
                            if let Ast::Symbol(_, id) = param {
                                params.push(id);
                            } else if let Ast::Bind(_, id) = param {
                                let name = lookup(id);
                                params.push(intern(&format!("&{}", name)));
                            } else {
                                return Err(SelError::SyntaxError(
                                    param.loc(),
                                    "Expected identifier in defmacro parameters".into(),
                                ));
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Expected parameter list in defmacro".into(),
                        ));
                    }
                }
                let body: Vec<Ast> = iter.collect();
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected identifier in defmacro".into(),
                    ));
                };
                Ok(Ast::DefMacro(
                    s_loc,
                    name_id,
                    Box::new(Ast::Lambda(s_loc, params, body)),
                ))
            }
            "set!" => {
                let mut iter = list.into_iter().skip(1);
                let Some(Ast::Symbol(_, name_id)) = iter.next() else {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected identifier in set!".into(),
                    ));
                };
                let value_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected expression in set!".into())
                })?;
                Ok(Ast::Set(s_loc, name_id, Box::new(value_ast)))
            }
            "let" | "let*" => {
                let is_star = lookup(id) == "let*";
                let mut iter = list.into_iter().skip(1);
                let bindings_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected bindings in let".into())
                })?;
                let mut bindings = Vec::new();
                match bindings_ast {
                    Ast::List(loc, b) => {
                        for bind in b {
                            if let Ast::List(loc, mut pair) = bind {
                                if pair.len() != 2 {
                                    return Err(SelError::SyntaxError(
                                        loc,
                                        "Invalid binding pair in let".into(),
                                    ));
                                }
                                let val = pair.pop().unwrap();
                                let name = pair.pop().unwrap();
                                if let Ast::Symbol(_, name_id) = name {
                                    bindings.push((name_id, val));
                                } else {
                                    return Err(SelError::SyntaxError(
                                        loc,
                                        "Expected identifier in let binding".into(),
                                    ));
                                }
                            } else {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected binding pair in let".into(),
                                ));
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Expected binding list in let".into(),
                        ));
                    }
                }
                let body: Vec<Ast> = iter.collect();
                if is_star && bindings.len() > 1 {
                    let mut current_body = body;
                    for (b_id, b_val) in bindings.into_iter().rev() {
                        current_body = vec![Ast::Let(s_loc, vec![(b_id, b_val)], current_body)];
                    }
                    Ok(current_body.pop().unwrap())
                } else {
                    Ok(Ast::Let(s_loc, bindings, body))
                }
            }
            "quote" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected expression in quote".into())
                })?;
                Ok(Ast::Quote(s_loc, Box::new(expr)))
            }
            "quasiquote" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected expression in quasiquote".into())
                })?;
                Ok(Ast::Quasiquote(s_loc, Box::new(expr)))
            }
            "and" => {
                let iter = list.into_iter().skip(1);
                Ok(Ast::And(s_loc, iter.collect()))
            }
            "or" => {
                let iter = list.into_iter().skip(1);
                Ok(Ast::Or(s_loc, iter.collect()))
            }
            "match" => {
                let mut iter = list.into_iter().skip(1);
                let target = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected target expression in match".into())
                })?;
                let mut clauses = Vec::new();
                for clause_ast in iter {
                    clauses.push(parse_match_clause(clause_ast)?);
                }
                if clauses.is_empty() {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected at least one clause in match".into(),
                    ));
                }
                Ok(Ast::Match(s_loc, Box::new(target), clauses))
            }
            _ => Ok(Ast::List(loc, list)),
        }
    } else {
        Ok(Ast::List(loc, list))
    }
}

pub fn parse_all(line: &str, file_id: u32, diags: &mut Vec<SelError>) -> Vec<Ast> {
    let mut lex = Lexer::new(line, file_id);
    let mut tokens = Vec::new();
    loop {
        match lex.next_token() {
            Ok(Some(t)) => tokens.push(t),
            Ok(None) => break,
            Err(e) => {
                diags.push(e);
            }
        }
    }
    let mut pos = 0;
    let mut asts = Vec::new();
    while pos < tokens.len() {
        match parse_expr(&tokens, &mut pos, diags) {
            Ok(ast) => asts.push(ast),
            Err(e) => {
                diags.push(e);
                recover_parser_state(&tokens, &mut pos);
            }
        }
    }
    asts
}

pub fn parse_expr(tokens: &[Token], pos: &mut usize, diags: &mut Vec<SelError>) -> Result<Ast> {
    if *pos >= tokens.len() {
        return Err(SelError::UnexpectedEOF(
            tokens.last().map(|t| t.loc).unwrap_or_default(),
        ));
    }
    let t = &tokens[*pos];
    *pos += 1;

    match t.kind {
        TokenKind::Bind => Err(SelError::SyntaxError(t.loc, "Unexpected `:=`".to_string())),
        TokenKind::BackSlash => parse_lambda_shorthand(tokens, pos, t, diags),
        TokenKind::OpenCurly => parse_record(tokens, pos, t, diags),
        TokenKind::OpenParen => parse_list(tokens, pos, t, diags),
        TokenKind::CloseParen => Err(SelError::SyntaxError(t.loc, "Unexpected `)`".to_string())),
        TokenKind::CloseCurly => Err(SelError::SyntaxError(t.loc, "Unexpected `}`".to_string())),
        TokenKind::Quote => {
            let expr = parse_expr(tokens, pos, diags)?;
            Ok(Ast::Quote(t.loc, Box::new(expr)))
        }
        TokenKind::Ampersand => {
            let expr = parse_expr(tokens, pos, diags)?;
            if let Ast::Symbol(loc, id) = expr {
                return Ok(Ast::Bind(loc, id));
            }
            Err(SelError::SyntaxError(
                t.loc,
                "Expected identifier after &".into(),
            ))
        }
        TokenKind::QuasiQuote => {
            let expr = parse_expr(tokens, pos, diags)?;
            Ok(Ast::Quasiquote(t.loc, Box::new(expr)))
        }
        TokenKind::Unquote => {
            let expr = parse_expr(tokens, pos, diags)?;
            Ok(Ast::Unquote(t.loc, Box::new(expr)))
        }
        TokenKind::UnquoteSplicing => {
            let expr = parse_expr(tokens, pos, diags)?;
            Ok(Ast::UnquoteSplicing(t.loc, Box::new(expr)))
        }
        TokenKind::Identifier => match t.source.as_str() {
            "nil" => Ok(Ast::Nil(t.loc)),
            ":private" => Ok(Ast::VisibilityDirective(t.loc, false)),
            ":public" => Ok(Ast::VisibilityDirective(t.loc, true)),
            _ => {
                if let Some(tb) = tokens.get(*pos)
                    && tb.kind == TokenKind::Bind
                {
                    *pos += 1;
                    let expr = parse_expr(tokens, pos, diags)?;
                    Ok(Ast::Define(t.loc, intern(&t.source), Box::new(expr)))
                } else {
                    Ok(Ast::Symbol(t.loc, intern(&t.source)))
                }
            }
        },
        TokenKind::Number(base) => {
            let s = match base {
                NumberBase::X => t.source.trim_start_matches("0x").trim_start_matches("0X"),
                NumberBase::B => t.source.trim_start_matches("0b").trim_start_matches("0B"),
                NumberBase::O => t.source.trim_start_matches("0o").trim_start_matches("0O"),
                NumberBase::D => &t.source,
            };

            if let Ok(i) = i64::from_str_radix(s, base.radix()) {
                return Ok(Ast::Integer(t.loc, i));
            } else if base == NumberBase::D
                && let Ok(f) = t.source.parse::<f64>()
            {
                return Ok(Ast::Float(t.loc, f));
            }
            Err(SelError::InvalidNumber(t.clone()))
        }
        TokenKind::String => Ok(Ast::String(t.loc, t.source.clone())),
        TokenKind::Boolean => Ok(Ast::Boolean(t.loc, t.source == "#t" || t.source == "#true")),
        TokenKind::Char(c) => Ok(Ast::Char(t.loc, c)),
    }
}

fn parse_lambda_shorthand(
    tokens: &[Token],
    pos: &mut usize,
    open_token: &Token,
    diags: &mut Vec<SelError>,
) -> Result<Ast> {
    let args = parse_expr(tokens, pos, diags)?;
    let body = parse_expr(tokens, pos, diags)?;
    Ok(Ast::List(
        open_token.loc,
        vec![Ast::Symbol(open_token.loc, intern("lambda")), args, body],
    ))
}

fn parse_record(
    tokens: &[Token],
    pos: &mut usize,
    open_token: &Token,
    diags: &mut Vec<SelError>,
) -> Result<Ast> {
    let mut record = Vec::new();
    while *pos < tokens.len() && tokens[*pos].kind != TokenKind::CloseCurly {
        let sym_expr = match parse_expr(tokens, pos, diags) {
            Ok(ast) => ast,
            Err(e) => {
                diags.push(e);
                recover_parser_state(tokens, pos);
                continue;
            }
        };
        let sym = match sym_expr {
            Ast::Symbol(_, sym) => sym,
            ast => {
                diags.push(SelError::SyntaxError(
                    ast.loc(),
                    format!("Expected identifier-value pair in records found {ast}"),
                ));
                recover_parser_state(tokens, pos);
                continue;
            }
        };
        let v_expr = match parse_expr(tokens, pos, diags) {
            Ok(ast) => ast,
            Err(e) => {
                diags.push(e);
                recover_parser_state(tokens, pos);
                continue;
            }
        };
        record.push((sym, v_expr));
    }
    if *pos >= tokens.len() {
        return Err(SelError::SyntaxError(
            open_token.loc,
            "Missing closing curly brace".into(),
        ));
    }
    *pos += 1; // consume '}'
    Ok(Ast::Record(open_token.loc, record))
}

fn parse_list(
    tokens: &[Token],
    pos: &mut usize,
    open_token: &Token,
    diags: &mut Vec<SelError>,
) -> Result<Ast> {
    let list = parse_list_expr(tokens, pos, open_token, diags)?;
    Ok(Ast::List(open_token.loc, list))
}

fn parse_list_expr(
    tokens: &[Token],
    pos: &mut usize,
    open_token: &Token,
    diags: &mut Vec<SelError>,
) -> Result<Vec<Ast>> {
    let mut list = Vec::new();
    while *pos < tokens.len() && tokens[*pos].kind != TokenKind::CloseParen {
        match parse_expr(tokens, pos, diags) {
            Ok(ast) => list.push(ast),
            Err(e) => {
                diags.push(e);
                recover_parser_state(tokens, pos);
            }
        }
    }
    if *pos >= tokens.len() {
        return Err(SelError::SyntaxError(
            open_token.loc,
            "Missing closing parenthesis".into(),
        ));
    }
    *pos += 1;
    Ok(list)
}

fn recover_parser_state(tokens: &[Token], pos: &mut usize) {
    let mut depth = 0;
    while *pos < tokens.len() {
        let t = &tokens[*pos];
        match t.kind {
            TokenKind::OpenParen | TokenKind::OpenCurly => {
                depth += 1;
                *pos += 1;
            }
            TokenKind::CloseParen | TokenKind::CloseCurly => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                *pos += 1;
            }
            _ => {
                if depth == 0 {
                    break;
                }
                *pos += 1;
            }
        }
    }
}

pub fn resolve_ast(ast: Ast) -> Result<Ast> {
    match ast {
        Ast::List(loc, list) => {
            if list.is_empty() {
                return Ok(Ast::Nil(loc));
            }
            if let Some(Ast::Symbol(_, sym_id)) = list.first() {
                let s = lookup(*sym_id);
                if matches!(
                    s.as_str(),
                    "match"
                        | "try"
                        | "->"
                        | "|>"
                        | "cond"
                        | "defn"
                        | "match-lambda"
                        | "with"
                        | "for"
                        | "let"
                        | "let*"
                        | "lambda"
                        | "fn"
                        | "define"
                        | "defmacro"
                        | "delay"
                        | "assert"
                ) {
                    let opt_ast = optimize_ast(list, loc)?;
                    return resolve_ast(opt_ast);
                }
            }
            let mut resolved_list = Vec::with_capacity(list.len());
            for item in list {
                resolved_list.push(resolve_ast(item)?);
            }
            optimize_ast(resolved_list, loc)
        }
        Ast::Quasiquote(loc, expr) => {
            Ok(Ast::Quasiquote(loc, Box::new(resolve_quasiquote(*expr)?)))
        }
        Ast::Unquote(loc, expr) => Ok(Ast::Unquote(loc, Box::new(resolve_ast(*expr)?))),
        Ast::UnquoteSplicing(loc, expr) => {
            Ok(Ast::UnquoteSplicing(loc, Box::new(resolve_ast(*expr)?)))
        }
        Ast::Record(loc, fields) => {
            let mut resolved_fields = Vec::with_capacity(fields.len());
            for (k, v) in fields {
                resolved_fields.push((k, resolve_ast(v)?));
            }
            Ok(Ast::Record(loc, resolved_fields))
        }
        Ast::Define(loc, id, expr) => Ok(Ast::Define(loc, id, Box::new(resolve_ast(*expr)?))),
        Ast::Set(loc, id, expr) => Ok(Ast::Set(loc, id, Box::new(resolve_ast(*expr)?))),
        Ast::Let(loc, bindings, body) => {
            let mut resolved_bindings = Vec::with_capacity(bindings.len());
            for (id, val) in bindings {
                resolved_bindings.push((id, resolve_ast(val)?));
            }
            let mut resolved_body = Vec::with_capacity(body.len());
            for expr in body {
                resolved_body.push(resolve_ast(expr)?);
            }
            Ok(Ast::Let(loc, resolved_bindings, resolved_body))
        }
        Ast::When(loc, cond, body) => {
            let resolved_cond = resolve_ast(*cond)?;
            let mut resolved_body = Vec::with_capacity(body.len());
            for expr in body {
                resolved_body.push(resolve_ast(expr)?);
            }
            Ok(Ast::When(loc, Box::new(resolved_cond), resolved_body))
        }
        Ast::Unless(loc, cond, false_branch, true_branch) => {
            let resolved_cond = resolve_ast(*cond)?;
            let resolved_false = resolve_ast(*false_branch)?;
            let resolved_true = match true_branch {
                Some(b) => Some(Box::new(resolve_ast(*b)?)),
                None => None,
            };
            Ok(Ast::Unless(
                loc,
                Box::new(resolved_cond),
                Box::new(resolved_false),
                resolved_true,
            ))
        }
        Ast::If(loc, cond, true_branch, false_branch) => {
            let resolved_cond = resolve_ast(*cond)?;
            let resolved_true = resolve_ast(*true_branch)?;
            let resolved_false = match false_branch {
                Some(b) => Some(Box::new(resolve_ast(*b)?)),
                None => None,
            };
            Ok(Ast::If(
                loc,
                Box::new(resolved_cond),
                Box::new(resolved_true),
                resolved_false,
            ))
        }
        Ast::Try(loc, body, err_var, catch_body) => {
            let resolved_body = resolve_ast(*body)?;
            let mut resolved_catch = Vec::with_capacity(catch_body.len());
            for expr in catch_body {
                resolved_catch.push(resolve_ast(expr)?);
            }
            Ok(Ast::Try(
                loc,
                Box::new(resolved_body),
                err_var,
                resolved_catch,
            ))
        }
        Ast::Lambda(loc, params, body) => {
            let mut resolved_body = Vec::with_capacity(body.len());
            for expr in body {
                resolved_body.push(resolve_ast(expr)?);
            }
            Ok(Ast::Lambda(loc, params, resolved_body))
        }
        Ast::DefMacro(loc, id, expr) => Ok(Ast::DefMacro(loc, id, Box::new(resolve_ast(*expr)?))),
        Ast::Begin(loc, body) => {
            let mut resolved_body = Vec::with_capacity(body.len());
            for expr in body {
                resolved_body.push(resolve_ast(expr)?);
            }
            Ok(Ast::Begin(loc, resolved_body))
        }
        Ast::Cond(loc, branches) => {
            let mut resolved_branches = Vec::with_capacity(branches.len());
            for (c, e) in branches {
                resolved_branches.push((resolve_ast(c)?, resolve_ast(e)?));
            }
            Ok(Ast::Cond(loc, resolved_branches))
        }
        Ast::While(loc, cond, body) => {
            let resolved_cond = resolve_ast(*cond)?;
            let resolved_body = match *body {
                Ast::List(b_loc, exprs) => {
                    let mut r = Vec::with_capacity(exprs.len());
                    for e in exprs {
                        r.push(resolve_ast(e)?);
                    }
                    Ast::List(b_loc, r)
                }
                other => resolve_ast(other)?,
            };
            Ok(Ast::While(
                loc,
                Box::new(resolved_cond),
                Box::new(resolved_body),
            ))
        }
        Ast::Until(loc, cond, body) => {
            let resolved_cond = resolve_ast(*cond)?;
            let resolved_body = match *body {
                Ast::List(b_loc, exprs) => {
                    let mut r = Vec::with_capacity(exprs.len());
                    for e in exprs {
                        r.push(resolve_ast(e)?);
                    }
                    Ast::List(b_loc, r)
                }
                other => resolve_ast(other)?,
            };
            Ok(Ast::Until(
                loc,
                Box::new(resolved_cond),
                Box::new(resolved_body),
            ))
        }
        Ast::And(loc, exprs) => {
            let mut r = Vec::with_capacity(exprs.len());
            for e in exprs {
                r.push(resolve_ast(e)?);
            }
            Ok(Ast::And(loc, r))
        }
        Ast::Or(loc, exprs) => {
            let mut r = Vec::with_capacity(exprs.len());
            for e in exprs {
                r.push(resolve_ast(e)?);
            }
            Ok(Ast::Or(loc, r))
        }
        Ast::Yield(loc, expr) => Ok(Ast::Yield(loc, Box::new(resolve_ast(*expr)?))),
        Ast::CoResume(loc, co, arg) => Ok(Ast::CoResume(
            loc,
            Box::new(resolve_ast(*co)?),
            Box::new(resolve_ast(*arg)?),
        )),
        Ast::Load(loc, path) => Ok(Ast::Load(loc, Box::new(resolve_ast(*path)?))),
        Ast::Match(loc, target, clauses) => {
            let resolved_target = resolve_ast(*target)?;
            let mut resolved_clauses = Vec::with_capacity(clauses.len());
            for c in clauses {
                let resolved_guard = match c.guard {
                    Some(g) => Some(resolve_ast(g)?),
                    None => None,
                };
                let mut resolved_body = Vec::with_capacity(c.body.len());
                for b in c.body {
                    resolved_body.push(resolve_ast(b)?);
                }
                resolved_clauses.push(MatchClause {
                    loc: c.loc,
                    pattern: c.pattern,
                    guard: resolved_guard,
                    body: resolved_body,
                });
            }
            Ok(Ast::Match(loc, Box::new(resolved_target), resolved_clauses))
        }
        other => Ok(other),
    }
}

pub fn parse_pattern(ast: Ast) -> Result<Pattern> {
    match ast {
        Ast::Symbol(loc, id) => {
            let name = lookup(id);
            if name == "_" {
                Ok(Pattern::Wildcard(loc))
            } else if name == "nil" {
                Ok(Pattern::Literal(loc, Box::new(Ast::Nil(loc))))
            } else {
                Ok(Pattern::Variable(loc, id))
            }
        }
        Ast::Bind(loc, id) => {
            let name = lookup(id);
            if name == "_" {
                Ok(Pattern::Wildcard(loc))
            } else {
                Ok(Pattern::Variable(loc, id))
            }
        }
        Ast::Nil(loc) => Ok(Pattern::Literal(loc, Box::new(Ast::Nil(loc)))),
        Ast::Integer(loc, i) => Ok(Pattern::Literal(loc, Box::new(Ast::Integer(loc, i)))),
        Ast::Float(loc, f) => Ok(Pattern::Literal(loc, Box::new(Ast::Float(loc, f)))),
        Ast::String(loc, s) => Ok(Pattern::Literal(loc, Box::new(Ast::String(loc, s)))),
        Ast::Boolean(loc, b) => Ok(Pattern::Literal(loc, Box::new(Ast::Boolean(loc, b)))),
        Ast::Char(loc, c) => Ok(Pattern::Literal(loc, Box::new(Ast::Char(loc, c)))),
        Ast::Quote(loc, val) => Ok(Pattern::Literal(loc, Box::new(Ast::Quote(loc, val)))),
        Ast::Record(loc, fields) => {
            let mut parsed_fields = Vec::with_capacity(fields.len());
            for (k, v_ast) in fields {
                parsed_fields.push((k, parse_pattern(v_ast)?));
            }
            Ok(Pattern::Record(loc, parsed_fields))
        }
        Ast::List(loc, items) => {
            if items.is_empty() {
                return Ok(Pattern::Literal(loc, Box::new(Ast::Nil(loc))));
            }
            let head_sym = if let Ast::Symbol(s_loc, id) = items[0] {
                Some((s_loc, id))
            } else {
                None
            };
            if let Some((s_loc, id)) = head_sym {
                match lookup(id).as_str() {
                    "quote" => {
                        let mut iter = items.into_iter().skip(1);
                        let expr = iter.next().ok_or_else(|| {
                            SelError::SyntaxError(
                                s_loc,
                                "Expected expression in quote pattern".into(),
                            )
                        })?;
                        return Ok(Pattern::Literal(
                            loc,
                            Box::new(Ast::Quote(loc, Box::new(expr))),
                        ));
                    }
                    "cons" => {
                        if items.len() != 3 {
                            return Err(SelError::SyntaxError(
                                s_loc,
                                "Expected 2 arguments for cons pattern: (cons head tail)".into(),
                            ));
                        }
                        let mut iter = items.into_iter().skip(1);
                        let h = parse_pattern(iter.next().unwrap())?;
                        let t = parse_pattern(iter.next().unwrap())?;
                        return Ok(Pattern::Cons(loc, Box::new(h), Box::new(t)));
                    }
                    "or" => {
                        let sub_pats = items
                            .into_iter()
                            .skip(1)
                            .map(parse_pattern)
                            .collect::<Result<Vec<_>>>()?;
                        if sub_pats.is_empty() {
                            return Err(SelError::SyntaxError(
                                s_loc,
                                "Expected at least 1 pattern in or pattern".into(),
                            ));
                        }
                        return Ok(Pattern::Or(loc, sub_pats));
                    }
                    "list" => {
                        let inner_items: Vec<Ast> = items.into_iter().skip(1).collect();
                        return parse_list_or_rest_pattern(loc, inner_items);
                    }
                    _ => {}
                }
            }
            parse_list_or_rest_pattern(loc, items)
        }
        other => Err(SelError::SyntaxError(
            other.loc(),
            format!("Invalid pattern: {}", other),
        )),
    }
}

fn parse_list_or_rest_pattern(loc: Loc, items: Vec<Ast>) -> Result<Pattern> {
    if items.is_empty() {
        return Ok(Pattern::List(loc, Vec::new()));
    }
    // Check if the last item is Ast::Bind (which is &identifier from lexer)
    if let Some(Ast::Bind(b_loc, id)) = items.last() {
        let b_loc = *b_loc;
        let id = *id;
        let prefix_items = &items[..items.len() - 1];
        let mut prefix = Vec::with_capacity(prefix_items.len());
        for item in prefix_items {
            prefix.push(parse_pattern(item.clone())?);
        }
        let rest = if lookup(id) == "_" {
            Pattern::Wildcard(b_loc)
        } else {
            Pattern::Variable(b_loc, id)
        };
        return Ok(Pattern::Rest(loc, prefix, Box::new(rest)));
    }
    // Check if second-to-last item is symbol "&"
    if items.len() >= 2
        && let Ast::Symbol(_, id) = &items[items.len() - 2]
        && lookup(*id) == "&"
    {
        let prefix_items = &items[..items.len() - 2];
        let mut prefix = Vec::with_capacity(prefix_items.len());
        for item in prefix_items {
            prefix.push(parse_pattern(item.clone())?);
        }
        let rest = parse_pattern(items.last().unwrap().clone())?;
        return Ok(Pattern::Rest(loc, prefix, Box::new(rest)));
    }
    let mut pats = Vec::with_capacity(items.len());
    for item in items {
        pats.push(parse_pattern(item)?);
    }
    Ok(Pattern::List(loc, pats))
}

pub fn parse_match_clause(ast: Ast) -> Result<MatchClause> {
    let Ast::List(c_loc, mut items) = ast else {
        return Err(SelError::SyntaxError(
            ast.loc(),
            "Expected clause to be a list in match".into(),
        ));
    };
    if items.is_empty() {
        return Err(SelError::SyntaxError(c_loc, "Empty clause in match".into()));
    }
    let pat_ast = items.remove(0);
    let pattern = parse_pattern(pat_ast)?;

    let mut guard = None;
    if !items.is_empty() {
        if let Ast::Symbol(_, sym_id) = &items[0] {
            let sym_name = lookup(*sym_id);
            if sym_name == ":where" || sym_name == ":when" {
                items.remove(0);
                if items.is_empty() {
                    return Err(SelError::SyntaxError(
                        c_loc,
                        "Expected guard condition after guard keyword".into(),
                    ));
                }
                guard = Some(items.remove(0));
            }
        } else if let Ast::When(_, cond, b) = &items[0] {
            if b.is_empty() {
                guard = Some((**cond).clone());
                items.remove(0);
            }
        } else if let Ast::List(g_loc, g_items) = &items[0]
            && !g_items.is_empty()
            && let Ast::Symbol(_, sym_id) = &g_items[0]
        {
            let sym_name = lookup(*sym_id);
            if sym_name == "where" || sym_name == "when" {
                if g_items.len() != 2 {
                    return Err(SelError::SyntaxError(
                        *g_loc,
                        "Expected (where condition) or (when condition)".into(),
                    ));
                }
                guard = Some(g_items[1].clone());
                items.remove(0);
            }
        }
    }

    if !items.is_empty()
        && let Ast::Symbol(_, sym_id) = &items[0]
        && lookup(*sym_id) == ":do"
    {
        items.remove(0);
    }

    let body = if items.is_empty() {
        vec![Ast::Nil(c_loc)]
    } else {
        items
    };
    Ok(MatchClause {
        loc: c_loc,
        pattern,
        guard,
        body,
    })
}

pub fn resolve_quasiquote(ast: Ast) -> Result<Ast> {
    match ast {
        Ast::Unquote(loc, expr) => Ok(Ast::Unquote(loc, Box::new(resolve_ast(*expr)?))),
        Ast::UnquoteSplicing(loc, expr) => {
            Ok(Ast::UnquoteSplicing(loc, Box::new(resolve_ast(*expr)?)))
        }
        Ast::List(loc, list) => {
            let mut resolved = Vec::with_capacity(list.len());
            for item in list {
                resolved.push(resolve_quasiquote(item)?);
            }
            Ok(Ast::List(loc, resolved))
        }
        Ast::Record(loc, fields) => {
            let mut resolved = Vec::with_capacity(fields.len());
            for (k, v) in fields {
                resolved.push((k, resolve_quasiquote(v)?));
            }
            Ok(Ast::Record(loc, resolved))
        }
        Ast::Load(loc, path) => Ok(Ast::Load(loc, Box::new(resolve_quasiquote(*path)?))),
        other => Ok(other),
    }
}

fn parse_try(list: Vec<Ast>, s_loc: Loc) -> Result<Ast> {
    let all_items: Vec<Ast> = list.into_iter().skip(1).collect();

    let mut iter = all_items.into_iter();
    let mut first = iter
        .next()
        .ok_or_else(|| SelError::SyntaxError(s_loc, "Missing body in try".into()))?;
    if let Ast::Symbol(_, s) = &first
        && lookup(*s) == ":do"
    {
        first = iter
            .next()
            .ok_or_else(|| SelError::SyntaxError(s_loc, "Missing body after :do in try".into()))?;
    }
    let catch_clause = iter
        .next()
        .ok_or_else(|| SelError::SyntaxError(s_loc, "Missing catch clause in try".into()))?;

    match catch_clause {
        Ast::List(c_loc, c_list) => {
            let mut c_iter = c_list.into_iter();
            let c_first = c_iter.next().ok_or_else(|| {
                SelError::SyntaxError(c_loc, "Expected (catch err-var ...) clause".into())
            })?;
            match c_first {
                Ast::Symbol(_, catch_sym_id) if lookup(catch_sym_id) == "catch" => {
                    let err_var = c_iter.next().ok_or_else(|| {
                        SelError::SyntaxError(
                            c_loc,
                            "Expected error variable in catch clause".into(),
                        )
                    })?;
                    let err_var_id = match err_var {
                        Ast::Symbol(_, id) => id,
                        _ => {
                            return Err(SelError::SyntaxError(
                                c_loc,
                                "Expected symbol for error variable".into(),
                            ));
                        }
                    };
                    let catch_body: Vec<Ast> = c_iter.collect();
                    Ok(Ast::Try(s_loc, Box::new(first), err_var_id, catch_body))
                }
                _ => Err(SelError::SyntaxError(
                    c_loc,
                    "Expected catch keyword as first element of catch clause".into(),
                )),
            }
        }
        _ => Err(SelError::SyntaxError(
            s_loc,
            "Expected catch clause to be a list".into(),
        )),
    }
}

fn parse_defn(list: Vec<Ast>, s_loc: Loc) -> Result<Ast> {
    let mut iter = list.into_iter().skip(1);
    let sig = iter
        .next()
        .ok_or_else(|| SelError::SyntaxError(s_loc, "Expected signature in defn".into()))?;
    let mut clauses = Vec::new();
    for clause_ast in iter {
        clauses.push(parse_match_clause(clause_ast)?);
    }
    if clauses.is_empty() {
        return Err(SelError::SyntaxError(
            s_loc,
            "Expected at least one clause in defn".into(),
        ));
    }
    match sig {
        Ast::Symbol(_, name_id) => {
            let arg_sym = gen_parser_id("_defn_arg");
            let match_ast = Ast::Match(s_loc, Box::new(Ast::Symbol(s_loc, arg_sym)), clauses);
            Ok(Ast::Define(
                s_loc,
                name_id,
                Box::new(Ast::Lambda(s_loc, vec![arg_sym], vec![match_ast])),
            ))
        }
        Ast::List(sig_loc, mut sig_items) => {
            if sig_items.is_empty() {
                return Err(SelError::SyntaxError(
                    sig_loc,
                    "Empty signature in defn".into(),
                ));
            }
            let head = sig_items.remove(0);
            let Ast::Symbol(_, name_id) = head else {
                return Err(SelError::SyntaxError(
                    sig_loc,
                    "Expected function name symbol in defn signature".into(),
                ));
            };
            let mut params = Vec::new();
            let mut arg_exprs = Vec::new();
            for p in sig_items {
                match p {
                    Ast::Symbol(p_loc, id) => {
                        params.push(id);
                        arg_exprs.push(Ast::Symbol(p_loc, id));
                    }
                    Ast::Bind(p_loc, id) => {
                        let name = lookup(id);
                        params.push(intern(&format!("&{}", name)));
                        arg_exprs.push(Ast::Symbol(p_loc, id));
                    }
                    _ => {
                        return Err(SelError::SyntaxError(
                            sig_loc,
                            "Expected parameter identifier in defn signature".into(),
                        ));
                    }
                }
            }
            let target_ast = if arg_exprs.len() == 1 {
                arg_exprs.remove(0)
            } else {
                let mut list_call = vec![Ast::Symbol(sig_loc, intern("list"))];
                list_call.extend(arg_exprs);
                Ast::List(sig_loc, list_call)
            };
            let match_ast = Ast::Match(s_loc, Box::new(target_ast), clauses);
            Ok(Ast::Define(
                s_loc,
                name_id,
                Box::new(Ast::Lambda(s_loc, params, vec![match_ast])),
            ))
        }
        _ => Err(SelError::SyntaxError(
            s_loc,
            "Expected symbol or (name args...) signature in defn".into(),
        )),
    }
}

fn parse_with(list: Vec<Ast>, s_loc: Loc) -> Result<Ast> {
    let args: Vec<Ast> = list.into_iter().skip(1).collect();
    let (clauses_asts, body_asts, else_asts) =
        if let Some(do_pos) = args.iter().position(|a| is_sym(a, ":do")) {
            let before_do = args[..do_pos].to_vec();
            let after_do = args[do_pos + 1..].to_vec();
            let clauses = if before_do.len() == 1
                && let Ast::List(_, inner) = &before_do[0]
                && !inner.is_empty()
                && matches!(&inner[0], Ast::List(_, _))
            {
                inner.clone()
            } else {
                before_do
            };
            if let Some(else_pos) = after_do.iter().position(|a| is_sym(a, ":else")) {
                (
                    clauses,
                    after_do[..else_pos].to_vec(),
                    after_do[else_pos + 1..].to_vec(),
                )
            } else {
                (clauses, after_do, Vec::new())
            }
        } else if !args.is_empty()
            && let Ast::List(_, inner) = &args[0]
        {
            (inner.clone(), args[1..].to_vec(), Vec::new())
        } else {
            return Err(SelError::SyntaxError(
                s_loc,
                "with requires :do or clauses list".into(),
            ));
        };

    let err_sym = gen_parser_id("_with_err");
    let else_handler_sym = gen_parser_id("_with_else");
    let else_fn_def = if else_asts.is_empty() {
        Ast::Lambda(s_loc, vec![err_sym], vec![Ast::Symbol(s_loc, err_sym)])
    } else {
        let mut else_clauses = Vec::with_capacity(else_asts.len());
        for c in else_asts {
            else_clauses.push(parse_match_clause(c)?);
        }
        Ast::Lambda(
            s_loc,
            vec![err_sym],
            vec![Ast::Match(
                s_loc,
                Box::new(Ast::Symbol(s_loc, err_sym)),
                else_clauses,
            )],
        )
    };

    let mut current = if body_asts.is_empty() {
        Ast::Nil(s_loc)
    } else if body_asts.len() == 1 {
        body_asts.into_iter().next().unwrap()
    } else {
        Ast::Begin(s_loc, body_asts)
    };

    for c_ast in clauses_asts.into_iter().rev() {
        let Ast::List(c_loc, mut c_items) = c_ast else {
            return Err(SelError::SyntaxError(
                c_ast.loc(),
                "Expected clause list in with".into(),
            ));
        };
        let fail_sym = gen_parser_id("_with_fail");
        let fail_clause = MatchClause {
            loc: c_loc,
            pattern: Pattern::Variable(c_loc, fail_sym),
            guard: None,
            body: vec![Ast::List(
                c_loc,
                vec![
                    Ast::Symbol(c_loc, else_handler_sym),
                    Ast::Symbol(c_loc, fail_sym),
                ],
            )],
        };
        if c_items.len() == 2 {
            let expr = c_items.pop().unwrap();
            let pat = parse_pattern(c_items.pop().unwrap())?;
            let ok_clause = MatchClause {
                loc: c_loc,
                pattern: pat,
                guard: None,
                body: vec![current],
            };
            current = Ast::Match(c_loc, Box::new(expr), vec![ok_clause, fail_clause]);
        } else if c_items.len() == 4 {
            let expr = c_items.pop().unwrap();
            let guard_expr = c_items.pop().unwrap();
            let guard_kw = c_items.pop().unwrap();
            if !is_sym(&guard_kw, ":when")
                && !is_sym(&guard_kw, ":where")
                && !is_sym(&guard_kw, "when")
                && !is_sym(&guard_kw, "where")
            {
                return Err(SelError::SyntaxError(
                    c_loc,
                    "Expected :when or :where in with clause".into(),
                ));
            }
            let pat = parse_pattern(c_items.pop().unwrap())?;
            let ok_clause = MatchClause {
                loc: c_loc,
                pattern: pat,
                guard: Some(guard_expr),
                body: vec![current],
            };
            current = Ast::Match(c_loc, Box::new(expr), vec![ok_clause, fail_clause]);
        } else {
            return Err(SelError::SyntaxError(
                c_loc,
                "Invalid clause format in with".into(),
            ));
        }
    }

    Ok(Ast::Let(
        s_loc,
        vec![(else_handler_sym, else_fn_def)],
        vec![current],
    ))
}

fn parse_for(list: Vec<Ast>, s_loc: Loc) -> Result<Ast> {
    let args: Vec<Ast> = list.into_iter().skip(1).collect();
    let mut gens = Vec::new();
    let mut idx = 0;

    if !args.is_empty()
        && let Ast::List(_, inner) = &args[0]
        && !inner.is_empty()
        && matches!(&inner[0], Ast::List(_, _))
    {
        gens = inner.clone();
        idx = 1;
    } else {
        while idx < args.len() {
            if is_sym(&args[idx], ":do")
                || is_sym(&args[idx], ":let")
                || is_sym(&args[idx], ":when")
                || is_sym(&args[idx], ":where")
            {
                break;
            }
            gens.push(args[idx].clone());
            idx += 1;
        }
    }

    let mut let_bindings: Option<Ast> = None;
    let mut when_cond: Option<Ast> = None;
    let mut body_asts: Vec<Ast> = Vec::new();

    while idx < args.len() {
        if is_sym(&args[idx], ":do") {
            body_asts = args[idx + 1..].to_vec();
            break;
        } else if is_sym(&args[idx], ":let") {
            let lb = args.get(idx + 1).cloned().ok_or_else(|| {
                SelError::SyntaxError(s_loc, "Missing bindings after :let in for".into())
            })?;
            let_bindings = Some(lb);
            idx += 2;
        } else if is_sym(&args[idx], ":when") || is_sym(&args[idx], ":where") {
            let wc = args.get(idx + 1).cloned().ok_or_else(|| {
                SelError::SyntaxError(s_loc, "Missing condition after :when/:where in for".into())
            })?;
            when_cond = Some(wc);
            idx += 2;
        } else {
            return Err(SelError::SyntaxError(
                args[idx].loc(),
                "Unexpected token in for".into(),
            ));
        }
    }

    if gens.is_empty() {
        return Err(SelError::SyntaxError(
            s_loc,
            "for requires at least one generator".into(),
        ));
    }

    let core_body = if body_asts.is_empty() {
        Ast::Nil(s_loc)
    } else if body_asts.len() == 1 {
        body_asts.into_iter().next().unwrap()
    } else {
        Ast::Begin(s_loc, body_asts)
    };

    let mut current_loop: Option<Ast> = None;
    for (rev_idx, g_ast) in gens.into_iter().rev().enumerate() {
        let Ast::List(g_loc, mut g_items) = g_ast else {
            return Err(SelError::SyntaxError(
                g_ast.loc(),
                "Expected generator (pattern seq) in for".into(),
            ));
        };
        if g_items.len() != 2 {
            return Err(SelError::SyntaxError(
                g_loc,
                "Generator in for must have exactly 2 elements: (pattern seq)".into(),
            ));
        }
        let seq = g_items.pop().unwrap();
        let pat = parse_pattern(g_items.pop().unwrap())?;
        let item_sym = gen_parser_id("_for_item");
        let acc_sym = gen_parser_id("_for_acc");

        let match_body = if rev_idx == 0 {
            let cons_call = Ast::List(
                g_loc,
                vec![
                    Ast::Symbol(g_loc, intern("cons")),
                    core_body.clone(),
                    Ast::Symbol(g_loc, acc_sym),
                ],
            );
            let if_action = match &when_cond {
                Some(cond) => Ast::If(
                    g_loc,
                    Box::new(cond.clone()),
                    Box::new(cons_call),
                    Some(Box::new(Ast::Symbol(g_loc, acc_sym))),
                ),
                None => cons_call,
            };
            match &let_bindings {
                Some(lb) => optimize_ast(
                    vec![Ast::Symbol(g_loc, intern("let")), lb.clone(), if_action],
                    g_loc,
                )?,
                None => if_action,
            }
        } else {
            Ast::List(
                g_loc,
                vec![
                    Ast::Symbol(g_loc, intern("append")),
                    current_loop.take().unwrap(),
                    Ast::Symbol(g_loc, acc_sym),
                ],
            )
        };

        let match_ast = Ast::Match(
            g_loc,
            Box::new(Ast::Symbol(g_loc, item_sym)),
            vec![
                MatchClause {
                    loc: g_loc,
                    pattern: pat,
                    guard: None,
                    body: vec![match_body],
                },
                MatchClause {
                    loc: g_loc,
                    pattern: Pattern::Wildcard(g_loc),
                    guard: None,
                    body: vec![Ast::Symbol(g_loc, acc_sym)],
                },
            ],
        );

        current_loop = Some(Ast::List(
            g_loc,
            vec![
                Ast::Symbol(g_loc, intern("foldr")),
                Ast::Lambda(g_loc, vec![item_sym, acc_sym], vec![match_ast]),
                Ast::Quote(g_loc, Box::new(Ast::Nil(g_loc))),
                seq,
            ],
        ));
    }

    Ok(current_loop.unwrap())
}
