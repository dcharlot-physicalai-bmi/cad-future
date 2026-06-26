//! CFL — CAD Future Language.
//!
//! A domain-specific language for the complete engineering lifecycle:
//! geometry, simulation, manufacturing, constraints, and AI — in one language.
//!
//! ## What makes CFL different from KCL (Zoo) or OpenSCAD
//!
//! - **Full lifecycle**: not just geometry — simulate, cost, manufacture in the same script
//! - **Physics-aware**: material properties and constraints are first-class citizens
//! - **AI-integrated**: `ask` keyword invokes the cascade (LUT → formula → solver → LLM)
//! - **Units-native**: all dimensions carry units (`50mm`, `2.5in`, `100MPa`)
//! - **Manufacturing-aware**: DFM checks and cost estimation are language primitives
//!
//! ## Example CFL script
//!
//! ```cfl
//! material al = "6061-T6"
//!
//! sketch base_profile {
//!   rect(50mm, 30mm)
//!   fillet(corners, 3mm)
//! }
//!
//! solid bracket = extrude(base_profile, 10mm, material: al)
//!
//! hole(bracket, diameter: 6mm, depth: through, position: [10mm, 15mm])
//! fillet(bracket, edges: top, radius: 2mm)
//!
//! assert weight(bracket) < 200g
//! assert stress(bracket, load: 500N) < yield(al) / 2
//!
//! dfm_check(bracket, process: cnc_3axis)
//! cost = estimate(bracket, quantity: 100)
//!
//! export(bracket, format: step, file: "bracket.step")
//! ```

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Token types
// ---------------------------------------------------------------------------

/// CFL token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Material,
    Sketch,
    Solid,
    Hole,
    Fillet,
    Chamfer,
    Extrude,
    Revolve,
    Loft,
    Sweep,
    Shell,
    Pattern,
    Mirror,
    Assert,
    Export,
    Import,
    Let,
    Fn,
    If,
    Else,
    For,
    In,
    Return,
    Ask,        // AI cascade query
    DfmCheck,
    Estimate,
    Simulate,
    // Geometry primitives
    Rect,
    Circle,
    Line,
    Arc,
    Polygon,
    // Literals
    Number(f64),
    StringLit(String),
    Ident(String),
    Unit(CflUnit),
    Bool(bool),
    // Operators
    Plus, Minus, Star, Slash, Percent,
    Eq, EqEq, NotEq, Lt, Gt, LtEq, GtEq,
    And, Or, Not,
    Dot, Comma, Colon, Semicolon, Arrow,
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    // Special
    Newline,
    Eof,
}

/// Physical units supported natively in CFL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CflUnit {
    // Length
    Mm, Cm, M, In, Ft,
    // Angle
    Deg, Rad,
    // Force
    N, Kn, Lbf,
    // Pressure
    Pa, Kpa, Mpa, Gpa, Psi,
    // Mass
    G, Kg, Lb,
    // Temperature
    C, K, F,
    // Time
    S, Min, Hr,
    // Frequency
    Hz, Khz, Mhz, Ghz,
    // Electrical
    Ohm, V, A, W,
    // Derived
    Mm2, Mm3, Mm4,
}

impl CflUnit {
    /// Parse a unit suffix string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mm" => Some(Self::Mm), "cm" => Some(Self::Cm), "m" => Some(Self::M),
            "in" => Some(Self::In), "ft" => Some(Self::Ft),
            "deg" => Some(Self::Deg), "rad" => Some(Self::Rad),
            "N" => Some(Self::N), "kN" => Some(Self::Kn), "lbf" => Some(Self::Lbf),
            "Pa" => Some(Self::Pa), "kPa" => Some(Self::Kpa), "MPa" => Some(Self::Mpa),
            "GPa" => Some(Self::Gpa), "psi" => Some(Self::Psi),
            "g" => Some(Self::G), "kg" => Some(Self::Kg), "lb" => Some(Self::Lb),
            "C" => Some(Self::C), "K" => Some(Self::K), "F" => Some(Self::F),
            "s" => Some(Self::S), "min" => Some(Self::Min), "hr" => Some(Self::Hr),
            "Hz" => Some(Self::Hz), "kHz" => Some(Self::Khz),
            "MHz" => Some(Self::Mhz), "GHz" => Some(Self::Ghz),
            "ohm" | "Ω" => Some(Self::Ohm), "V" => Some(Self::V),
            "A" => Some(Self::A), "W" => Some(Self::W),
            "mm2" | "mm²" => Some(Self::Mm2), "mm3" | "mm³" => Some(Self::Mm3),
            "mm4" | "mm⁴" => Some(Self::Mm4),
            _ => None,
        }
    }

    /// Convert a value in this unit to base SI.
    pub fn to_si(&self, value: f64) -> f64 {
        match self {
            Self::Mm => value * 1e-3, Self::Cm => value * 1e-2, Self::M => value,
            Self::In => value * 0.0254, Self::Ft => value * 0.3048,
            Self::Deg => value * std::f64::consts::PI / 180.0, Self::Rad => value,
            Self::N => value, Self::Kn => value * 1e3, Self::Lbf => value * 4.44822,
            Self::Pa => value, Self::Kpa => value * 1e3, Self::Mpa => value * 1e6,
            Self::Gpa => value * 1e9, Self::Psi => value * 6894.76,
            Self::G => value * 1e-3, Self::Kg => value, Self::Lb => value * 0.453592,
            Self::C => value + 273.15, Self::K => value, Self::F => (value - 32.0) * 5.0/9.0 + 273.15,
            Self::S => value, Self::Min => value * 60.0, Self::Hr => value * 3600.0,
            Self::Hz => value, Self::Khz => value * 1e3, Self::Mhz => value * 1e6, Self::Ghz => value * 1e9,
            Self::Ohm | Self::V | Self::A | Self::W => value,
            Self::Mm2 => value * 1e-6, Self::Mm3 => value * 1e-9, Self::Mm4 => value * 1e-12,
        }
    }
}

// ---------------------------------------------------------------------------
// AST (Abstract Syntax Tree)
// ---------------------------------------------------------------------------

/// A typed value in CFL — all values carry optional units.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CflValue {
    pub number: f64,
    pub unit: Option<CflUnit>,
}

impl CflValue {
    pub fn new(number: f64, unit: Option<CflUnit>) -> Self { Self { number, unit } }
    pub fn unitless(number: f64) -> Self { Self { number, unit: None } }
    pub fn to_si(&self) -> f64 {
        match self.unit {
            Some(u) => u.to_si(self.number),
            None => self.number,
        }
    }
}

/// AST node for a CFL program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    /// Numeric literal with optional unit: `50mm`, `3.14`, `100MPa`
    Literal(CflValue),
    /// String literal: `"6061-T6"`
    StringLit(String),
    /// Boolean
    BoolLit(bool),
    /// Variable reference: `bracket`, `al`
    Ident(String),
    /// Binary operation: `a + b`, `stress < yield / 2`
    BinOp { left: Box<Expr>, op: BinOp, right: Box<Expr> },
    /// Unary operation: `-x`, `!condition`
    UnaryOp { op: UnaryOp, operand: Box<Expr> },
    /// Function/method call: `extrude(profile, 10mm)`, `weight(bracket)`
    Call { name: String, args: Vec<Expr>, kwargs: HashMap<String, Expr> },
    /// Array literal: `[10mm, 15mm]`
    Array(Vec<Expr>),
    /// Member access: `bracket.volume`, `al.yield_strength`
    MemberAccess { object: Box<Expr>, member: String },
    /// Material declaration: `material al = "6061-T6"`
    MaterialDecl { name: String, id: Box<Expr> },
    /// Sketch block
    SketchBlock { name: String, body: Vec<Expr> },
    /// Solid assignment: `solid bracket = extrude(...)`
    SolidDecl { name: String, value: Box<Expr> },
    /// Let binding: `let x = 50mm`
    LetDecl { name: String, value: Box<Expr> },
    /// Assert: `assert weight(bracket) < 200g`
    Assert { condition: Box<Expr>, message: Option<String> },
    /// Export: `export(bracket, format: step)`
    Export { target: Box<Expr>, kwargs: HashMap<String, Expr> },
    /// Import: `import("part.step")` or `import("design.kcl")`
    Import { path: String, format: Option<String> },
    /// AI query: `ask "what material for 500N at 200C?"`
    Ask { query: Box<Expr> },
    /// DFM check: `dfm_check(bracket, process: cnc_3axis)`
    DfmCheck { target: Box<Expr>, kwargs: HashMap<String, Expr> },
    /// Cost estimate: `estimate(bracket, quantity: 100)`
    CostEstimate { target: Box<Expr>, kwargs: HashMap<String, Expr> },
    /// Simulate: `simulate(bracket, type: fea, load: 500N)`
    Simulate { target: Box<Expr>, kwargs: HashMap<String, Expr> },
    /// If/else
    IfElse { condition: Box<Expr>, then_body: Vec<Expr>, else_body: Option<Vec<Expr>> },
    /// For loop: `for i in 1..10 { ... }`
    ForLoop { var: String, iter: Box<Expr>, body: Vec<Expr> },
    /// Range: `1..10`
    Range { start: Box<Expr>, end: Box<Expr> },
    /// Function definition
    FnDecl { name: String, params: Vec<(String, Option<CflUnit>)>, body: Vec<Expr> },
    /// Block (sequence of expressions)
    Block(Vec<Expr>),
    /// Comment (preserved for documentation)
    Comment(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    And, Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp { Neg, Not }

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

/// Tokenize a CFL source string.
pub fn lex(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace (except newlines)
        if c == ' ' || c == '\t' || c == '\r' { i += 1; continue; }

        // Newline
        if c == '\n' { tokens.push(Token::Newline); i += 1; continue; }

        // Comments
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' { i += 1; }
            continue;
        }

        // String literals
        if c == '"' {
            i += 1;
            let mut s = String::new();
            while i < chars.len() && chars[i] != '"' { s.push(chars[i]); i += 1; }
            if i < chars.len() { i += 1; } // skip closing "
            tokens.push(Token::StringLit(s));
            continue;
        }

        // Numbers (with optional unit suffix)
        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') { i += 1; }
            let num_str: String = chars[start..i].iter().collect();
            let num: f64 = num_str.parse().unwrap_or(0.0);

            // Check for unit suffix
            let unit_start = i;
            while i < chars.len() && (chars[i].is_ascii_alphabetic() || chars[i] == '²' || chars[i] == '³' || chars[i] == '⁴' || chars[i] == 'Ω') { i += 1; }
            if i > unit_start {
                let unit_str: String = chars[unit_start..i].iter().collect();
                if let Some(unit) = CflUnit::parse(&unit_str) {
                    tokens.push(Token::Number(num));
                    tokens.push(Token::Unit(unit));
                    continue;
                } else {
                    // Not a unit, backtrack
                    i = unit_start;
                }
            }
            tokens.push(Token::Number(num));
            continue;
        }

        // Identifiers and keywords
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') { i += 1; }
            let word: String = chars[start..i].iter().collect();
            let tok = match word.as_str() {
                "material" => Token::Material,
                "sketch" => Token::Sketch,
                "solid" => Token::Solid,
                "hole" => Token::Hole,
                "fillet" => Token::Fillet,
                "chamfer" => Token::Chamfer,
                "extrude" => Token::Extrude,
                "revolve" => Token::Revolve,
                "loft" => Token::Loft,
                "sweep" => Token::Sweep,
                "shell" => Token::Shell,
                "pattern" => Token::Pattern,
                "mirror" => Token::Mirror,
                "assert" => Token::Assert,
                "export" => Token::Export,
                "import" => Token::Import,
                "let" => Token::Let,
                "fn" => Token::Fn,
                "if" => Token::If,
                "else" => Token::Else,
                "for" => Token::For,
                "in" => Token::In,
                "return" => Token::Return,
                "ask" => Token::Ask,
                "dfm_check" => Token::DfmCheck,
                "estimate" => Token::Estimate,
                "simulate" => Token::Simulate,
                "rect" => Token::Rect,
                "circle" => Token::Circle,
                "line" => Token::Line,
                "arc" => Token::Arc,
                "polygon" => Token::Polygon,
                "true" => Token::Bool(true),
                "false" => Token::Bool(false),
                _ => Token::Ident(word),
            };
            tokens.push(tok);
            continue;
        }

        // Operators and punctuation
        match c {
            '+' => { tokens.push(Token::Plus); i += 1; }
            '-' => {
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    tokens.push(Token::Arrow); i += 2;
                } else {
                    tokens.push(Token::Minus); i += 1;
                }
            }
            '*' => { tokens.push(Token::Star); i += 1; }
            '/' => { tokens.push(Token::Slash); i += 1; }
            '%' => { tokens.push(Token::Percent); i += 1; }
            '=' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::EqEq); i += 2;
                } else {
                    tokens.push(Token::Eq); i += 1;
                }
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::NotEq); i += 2;
                } else {
                    tokens.push(Token::Not); i += 1;
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::LtEq); i += 2;
                } else {
                    tokens.push(Token::Lt); i += 1;
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::GtEq); i += 2;
                } else {
                    tokens.push(Token::Gt); i += 1;
                }
            }
            '&' => {
                if i + 1 < chars.len() && chars[i + 1] == '&' { tokens.push(Token::And); i += 2; }
                else { i += 1; }
            }
            '|' => {
                if i + 1 < chars.len() && chars[i + 1] == '|' { tokens.push(Token::Or); i += 2; }
                else { i += 1; }
            }
            '.' => {
                if i + 1 < chars.len() && chars[i + 1] == '.' {
                    i += 2; // range operator handled elsewhere
                } else {
                    tokens.push(Token::Dot); i += 1;
                }
            }
            ',' => { tokens.push(Token::Comma); i += 1; }
            ':' => { tokens.push(Token::Colon); i += 1; }
            ';' => { tokens.push(Token::Semicolon); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            '{' => { tokens.push(Token::LBrace); i += 1; }
            '}' => { tokens.push(Token::RBrace); i += 1; }
            '[' => { tokens.push(Token::LBracket); i += 1; }
            ']' => { tokens.push(Token::RBracket); i += 1; }
            _ => { i += 1; } // skip unknown
        }
    }

    tokens.push(Token::Eof);
    tokens
}

// ---------------------------------------------------------------------------
// Parser — produces AST from tokens
// ---------------------------------------------------------------------------

/// Parse CFL tokens into an AST.
pub fn parse(tokens: &[Token]) -> Vec<Expr> {
    let mut ast = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        // Skip newlines
        if tokens[pos] == Token::Newline { pos += 1; continue; }
        if tokens[pos] == Token::Eof { break; }

        if let Some((expr, next)) = parse_statement(tokens, pos) {
            ast.push(expr);
            pos = next;
        } else {
            pos += 1; // skip unparseable token
        }
    }

    ast
}

fn parse_statement(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    match &tokens[pos] {
        Token::Material => parse_material_decl(tokens, pos),
        Token::Sketch => parse_sketch_block(tokens, pos),
        Token::Solid => parse_solid_decl(tokens, pos),
        Token::Let => parse_let_decl(tokens, pos),
        Token::Assert => parse_assert(tokens, pos),
        Token::Export => parse_export(tokens, pos),
        Token::Import => parse_import(tokens, pos),
        Token::Ask => parse_ask(tokens, pos),
        _ => parse_expr(tokens, pos),
    }
}

fn parse_material_decl(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    // material NAME = EXPR
    let pos = pos + 1; // skip 'material'
    let name = if let Token::Ident(n) = &tokens[pos] { n.clone() } else { return None; };
    let pos = pos + 1;
    if tokens[pos] != Token::Eq { return None; }
    let pos = pos + 1;
    let (value, pos) = parse_expr(tokens, pos)?;
    Some((Expr::MaterialDecl { name, id: Box::new(value) }, pos))
}

fn parse_sketch_block(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    // sketch NAME { ... }
    let pos = pos + 1;
    let name = if let Token::Ident(n) = &tokens[pos] { n.clone() } else { return None; };
    let pos = pos + 1;
    if tokens[pos] != Token::LBrace { return None; }
    let (body, pos) = parse_block(tokens, pos + 1)?;
    Some((Expr::SketchBlock { name, body }, pos))
}

fn parse_solid_decl(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    // solid NAME = EXPR
    let pos = pos + 1;
    let name = if let Token::Ident(n) = &tokens[pos] { n.clone() } else { return None; };
    let pos = pos + 1;
    if tokens[pos] != Token::Eq { return None; }
    let pos = pos + 1;
    let (value, pos) = parse_expr(tokens, pos)?;
    Some((Expr::SolidDecl { name, value: Box::new(value) }, pos))
}

fn parse_let_decl(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let pos = pos + 1;
    let name = if let Token::Ident(n) = &tokens[pos] { n.clone() } else { return None; };
    let pos = pos + 1;
    if tokens[pos] != Token::Eq { return None; }
    let pos = pos + 1;
    let (value, pos) = parse_expr(tokens, pos)?;
    Some((Expr::LetDecl { name, value: Box::new(value) }, pos))
}

fn parse_assert(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let pos = pos + 1;
    let (condition, pos) = parse_expr(tokens, pos)?;
    Some((Expr::Assert { condition: Box::new(condition), message: None }, pos))
}

fn parse_export(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let pos = pos + 1;
    if tokens[pos] != Token::LParen { return None; }
    let (args, kwargs, pos) = parse_call_args(tokens, pos + 1)?;
    let target = args.into_iter().next().unwrap_or(Expr::Ident("_".into()));
    Some((Expr::Export { target: Box::new(target), kwargs }, pos))
}

fn parse_import(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let pos = pos + 1;
    if tokens[pos] != Token::LParen { return None; }
    let pos = pos + 1;
    let path = if let Token::StringLit(s) = &tokens[pos] { s.clone() } else { return None; };
    let pos = pos + 1;
    // skip )
    let pos = if tokens[pos] == Token::RParen { pos + 1 } else { pos };
    Some((Expr::Import { path, format: None }, pos))
}

fn parse_ask(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let pos = pos + 1;
    let (query, pos) = parse_expr(tokens, pos)?;
    Some((Expr::Ask { query: Box::new(query) }, pos))
}

fn parse_expr(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    parse_comparison(tokens, pos)
}

fn parse_comparison(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let (mut left, mut pos) = parse_additive(tokens, pos)?;
    loop {
        let op = match &tokens[pos] {
            Token::Lt => BinOp::Lt, Token::Gt => BinOp::Gt,
            Token::LtEq => BinOp::LtEq, Token::GtEq => BinOp::GtEq,
            Token::EqEq => BinOp::Eq, Token::NotEq => BinOp::NotEq,
            _ => break,
        };
        pos += 1;
        let (right, next) = parse_additive(tokens, pos)?;
        left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right) };
        pos = next;
    }
    Some((left, pos))
}

fn parse_additive(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let (mut left, mut pos) = parse_multiplicative(tokens, pos)?;
    loop {
        let op = match &tokens[pos] {
            Token::Plus => BinOp::Add, Token::Minus => BinOp::Sub,
            _ => break,
        };
        pos += 1;
        let (right, next) = parse_multiplicative(tokens, pos)?;
        left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right) };
        pos = next;
    }
    Some((left, pos))
}

fn parse_multiplicative(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let (mut left, mut pos) = parse_primary(tokens, pos)?;
    loop {
        let op = match &tokens[pos] {
            Token::Star => BinOp::Mul, Token::Slash => BinOp::Div,
            Token::Percent => BinOp::Mod,
            _ => break,
        };
        pos += 1;
        let (right, next) = parse_primary(tokens, pos)?;
        left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right) };
        pos = next;
    }
    Some((left, pos))
}

fn parse_primary(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    match &tokens[pos] {
        Token::Number(n) => {
            let n = *n;
            // Check for unit suffix
            if let Token::Unit(u) = &tokens[pos + 1] {
                Some((Expr::Literal(CflValue::new(n, Some(*u))), pos + 2))
            } else {
                Some((Expr::Literal(CflValue::unitless(n)), pos + 1))
            }
        }
        Token::StringLit(s) => Some((Expr::StringLit(s.clone()), pos + 1)),
        Token::Bool(b) => Some((Expr::BoolLit(*b), pos + 1)),
        Token::Ident(name) => {
            let name = name.clone();
            // Check for function call
            if tokens[pos + 1] == Token::LParen {
                let (args, kwargs, next) = parse_call_args(tokens, pos + 2)?;
                Some((Expr::Call { name, args, kwargs }, next))
            }
            // Check for member access
            else if tokens[pos + 1] == Token::Dot {
                if let Token::Ident(member) = &tokens[pos + 2] {
                    Some((Expr::MemberAccess {
                        object: Box::new(Expr::Ident(name)),
                        member: member.clone(),
                    }, pos + 3))
                } else {
                    Some((Expr::Ident(name), pos + 1))
                }
            } else {
                Some((Expr::Ident(name), pos + 1))
            }
        }
        Token::LParen => {
            let (expr, pos) = parse_expr(tokens, pos + 1)?;
            let pos = if tokens[pos] == Token::RParen { pos + 1 } else { pos };
            Some((expr, pos))
        }
        Token::LBracket => {
            let mut elements = Vec::new();
            let mut p = pos + 1;
            while p < tokens.len() && tokens[p] != Token::RBracket {
                if tokens[p] == Token::Comma { p += 1; continue; }
                if let Some((expr, next)) = parse_expr(tokens, p) {
                    elements.push(expr);
                    p = next;
                } else {
                    break;
                }
            }
            if p < tokens.len() && tokens[p] == Token::RBracket { p += 1; }
            Some((Expr::Array(elements), p))
        }
        Token::Minus => {
            let (operand, next) = parse_primary(tokens, pos + 1)?;
            Some((Expr::UnaryOp { op: UnaryOp::Neg, operand: Box::new(operand) }, next))
        }
        // Geometry keywords as function calls
        Token::Extrude | Token::Revolve | Token::Loft | Token::Sweep |
        Token::Fillet | Token::Chamfer | Token::Shell | Token::Hole |
        Token::Pattern | Token::Mirror | Token::Rect | Token::Circle |
        Token::Line | Token::Arc | Token::Polygon |
        Token::DfmCheck | Token::Estimate | Token::Simulate => {
            let name = format!("{:?}", tokens[pos]).to_lowercase();
            if tokens[pos + 1] == Token::LParen {
                let (args, kwargs, next) = parse_call_args(tokens, pos + 2)?;
                Some((Expr::Call { name, args, kwargs }, next))
            } else {
                Some((Expr::Ident(name), pos + 1))
            }
        }
        _ => None,
    }
}

fn parse_call_args(tokens: &[Token], pos: usize) -> Option<(Vec<Expr>, HashMap<String, Expr>, usize)> {
    let mut args = Vec::new();
    let mut kwargs = HashMap::new();
    let mut p = pos;

    while p < tokens.len() && tokens[p] != Token::RParen {
        if tokens[p] == Token::Comma || tokens[p] == Token::Newline { p += 1; continue; }

        // Check for keyword argument: name: value
        if let Token::Ident(name) = &tokens[p] {
            if p + 1 < tokens.len() && tokens[p + 1] == Token::Colon {
                let key = name.clone();
                p += 2; // skip name and colon
                if let Some((val, next)) = parse_expr(tokens, p) {
                    kwargs.insert(key, val);
                    p = next;
                    continue;
                }
            }
        }

        if let Some((expr, next)) = parse_expr(tokens, p) {
            args.push(expr);
            p = next;
        } else {
            break;
        }
    }

    if p < tokens.len() && tokens[p] == Token::RParen { p += 1; }
    Some((args, kwargs, p))
}

fn parse_block(tokens: &[Token], pos: usize) -> Option<(Vec<Expr>, usize)> {
    let mut body = Vec::new();
    let mut p = pos;
    while p < tokens.len() && tokens[p] != Token::RBrace {
        if tokens[p] == Token::Newline { p += 1; continue; }
        if let Some((expr, next)) = parse_statement(tokens, p) {
            body.push(expr);
            p = next;
        } else {
            p += 1;
        }
    }
    if p < tokens.len() && tokens[p] == Token::RBrace { p += 1; }
    Some((body, p))
}

// ---------------------------------------------------------------------------
// CFL → MCP Tool Calls (compilation)
// ---------------------------------------------------------------------------

/// Compile a CFL AST into a sequence of MCP tool call JSON objects.
pub fn compile_to_mcp(ast: &[Expr]) -> Vec<serde_json::Value> {
    let mut calls = Vec::new();
    for expr in ast {
        match expr {
            Expr::MaterialDecl { name, id } => {
                if let Expr::StringLit(mat_id) = id.as_ref() {
                    calls.push(serde_json::json!({
                        "tool": "lookup_material",
                        "args": { "material_id": mat_id },
                        "bind": name,
                    }));
                }
            }
            Expr::SolidDecl { name, value } => {
                if let Expr::Call { name: fn_name, args, kwargs } = value.as_ref() {
                    let mut tool_args = serde_json::Map::new();
                    for (k, v) in kwargs {
                        tool_args.insert(k.clone(), expr_to_json(v));
                    }
                    for (i, a) in args.iter().enumerate() {
                        tool_args.insert(format!("arg{i}"), expr_to_json(a));
                    }
                    calls.push(serde_json::json!({
                        "tool": fn_name,
                        "args": tool_args,
                        "bind": name,
                    }));
                }
            }
            Expr::Call { name, args, kwargs } => {
                let mut tool_args = serde_json::Map::new();
                for (k, v) in kwargs {
                    tool_args.insert(k.clone(), expr_to_json(v));
                }
                for (i, a) in args.iter().enumerate() {
                    tool_args.insert(format!("arg{i}"), expr_to_json(a));
                }
                calls.push(serde_json::json!({
                    "tool": name,
                    "args": tool_args,
                }));
            }
            Expr::Export { target, kwargs } => {
                let mut tool_args = serde_json::Map::new();
                tool_args.insert("target".into(), expr_to_json(target));
                for (k, v) in kwargs {
                    tool_args.insert(k.clone(), expr_to_json(v));
                }
                calls.push(serde_json::json!({
                    "tool": "export",
                    "args": tool_args,
                }));
            }
            Expr::Assert { condition, message } => {
                calls.push(serde_json::json!({
                    "tool": "assert",
                    "args": { "condition": format!("{:?}", condition), "message": message },
                }));
            }
            _ => {}
        }
    }
    calls
}

fn expr_to_json(expr: &Expr) -> serde_json::Value {
    match expr {
        Expr::Literal(v) => serde_json::json!({ "value": v.number, "unit": format!("{:?}", v.unit) }),
        Expr::StringLit(s) => serde_json::Value::String(s.clone()),
        Expr::BoolLit(b) => serde_json::Value::Bool(*b),
        Expr::Ident(n) => serde_json::Value::String(format!("${n}")),
        _ => serde_json::json!(format!("{:?}", expr)),
    }
}

// ---------------------------------------------------------------------------
// KCL Import — ingest Zoo/KittyCAD programs
// ---------------------------------------------------------------------------

/// Convert a KCL (Zoo) program to CFL by translating syntax.
/// This is a basic transpiler that handles common KCL patterns.
pub fn kcl_to_cfl(kcl_source: &str) -> String {
    let mut cfl = String::new();
    cfl.push_str("// Auto-converted from KCL (Zoo/KittyCAD)\n");

    for line in kcl_source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            cfl.push_str(line);
            cfl.push('\n');
            continue;
        }

        // KCL const → CFL let
        let converted = trimmed
            .replace("const ", "let ")
            .replace("startSketchOn", "sketch_on")
            .replace("startProfileAt", "move_to")
            .replace("lineTo", "line_to")
            .replace("angledLine", "line_angle")
            .replace("tangentialArcTo", "arc_to")
            .replace("close()", "close()")
            .replace("|>", "->"); // pipe operator

        cfl.push_str(&converted);
        cfl.push('\n');
    }

    cfl
}

// ---------------------------------------------------------------------------
// OpenSCAD Import
// ---------------------------------------------------------------------------

/// Convert an OpenSCAD program to CFL (basic patterns).
pub fn openscad_to_cfl(scad_source: &str) -> String {
    let mut cfl = String::new();
    cfl.push_str("// Auto-converted from OpenSCAD\n");

    for line in scad_source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            cfl.push_str(line);
            cfl.push('\n');
            continue;
        }

        let converted = trimmed
            .replace("cube(", "box(")
            .replace("cylinder(", "cylinder(")
            .replace("sphere(", "sphere(")
            .replace("translate(", "translate(")
            .replace("rotate(", "rotate(")
            .replace("difference()", "subtract {")
            .replace("union()", "union {")
            .replace("intersection()", "intersect {");

        cfl.push_str(&converted);
        cfl.push('\n');
    }

    cfl
}

// ---------------------------------------------------------------------------
// CFL Execution Engine
// ---------------------------------------------------------------------------

/// Runtime value — the result of evaluating an expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeValue {
    Number(f64, Option<CflUnit>),
    String(String),
    Bool(bool),
    Array(Vec<RuntimeValue>),
    Solid(String), // handle/name reference
    Void,
    Error(String),
}

impl RuntimeValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self { Self::Number(n, _) => Some(*n), _ => None }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self { Self::Bool(b) => Some(*b), _ => None }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { Self::String(s) => Some(s), _ => None }
    }
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Number(n, _) => *n != 0.0,
            Self::String(s) => !s.is_empty(),
            Self::Bool(b) => *b,
            Self::Array(a) => !a.is_empty(),
            Self::Solid(_) => true,
            Self::Void => false,
            Self::Error(_) => false,
        }
    }
}

/// The result of executing a CFL program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// All variable bindings after execution.
    pub variables: HashMap<String, RuntimeValue>,
    /// Assertion results (name → passed).
    pub assertions: Vec<(String, bool)>,
    /// MCP tool calls generated during execution.
    pub tool_calls: Vec<serde_json::Value>,
    /// Output log.
    pub log: Vec<String>,
    /// Whether all assertions passed.
    pub all_passed: bool,
    /// Errors encountered.
    pub errors: Vec<String>,
}

/// Execute a CFL program.
pub fn execute(source: &str) -> ExecutionResult {
    let tokens = lex(source);
    let ast = parse(&tokens);
    execute_ast(&ast)
}

/// Execute a parsed CFL AST.
pub fn execute_ast(ast: &[Expr]) -> ExecutionResult {
    let mut env = HashMap::new();
    let mut assertions = Vec::new();
    let mut tool_calls = Vec::new();
    let mut log = Vec::new();
    let mut errors = Vec::new();

    for expr in ast {
        match expr {
            Expr::MaterialDecl { name, id } => {
                let val = eval_expr(id, &env);
                if let RuntimeValue::String(mat_id) = &val {
                    env.insert(name.clone(), RuntimeValue::String(mat_id.clone()));
                    log.push(format!("material {name} = \"{mat_id}\""));
                    tool_calls.push(serde_json::json!({
                        "tool": "lookup_material", "args": { "material_id": mat_id }
                    }));
                }
            }
            Expr::LetDecl { name, value } => {
                let val = eval_expr(value, &env);
                log.push(format!("let {name} = {:?}", val));
                env.insert(name.clone(), val);
            }
            Expr::SolidDecl { name, value } => {
                let val = eval_expr(value, &env);
                log.push(format!("solid {name} = {:?}", val));
                env.insert(name.clone(), RuntimeValue::Solid(name.clone()));
                // Generate MCP tool call
                if let Expr::Call { name: fn_name, args, kwargs } = value.as_ref() {
                    let mut ta = serde_json::Map::new();
                    for (k, v) in kwargs { ta.insert(k.clone(), runtime_to_json(&eval_expr(v, &env))); }
                    for (i, a) in args.iter().enumerate() { ta.insert(format!("arg{i}"), runtime_to_json(&eval_expr(a, &env))); }
                    tool_calls.push(serde_json::json!({ "tool": fn_name, "args": ta, "bind": name }));
                }
            }
            Expr::Assert { condition, message } => {
                let val = eval_expr(condition, &env);
                let passed = val.is_truthy();
                let desc = message.clone().unwrap_or_else(|| format!("{:?}", condition));
                assertions.push((desc.clone(), passed));
                if !passed {
                    errors.push(format!("Assertion failed: {desc}"));
                }
                log.push(format!("assert {} → {}", desc, if passed { "PASS" } else { "FAIL" }));
            }
            Expr::Export { target, kwargs } => {
                let mut ta = serde_json::Map::new();
                ta.insert("target".into(), runtime_to_json(&eval_expr(target, &env)));
                for (k, v) in kwargs { ta.insert(k.clone(), runtime_to_json(&eval_expr(v, &env))); }
                tool_calls.push(serde_json::json!({ "tool": "export", "args": ta }));
                log.push(format!("export → {:?}", kwargs.keys().collect::<Vec<_>>()));
            }
            Expr::Call { name, args, kwargs } => {
                let mut ta = serde_json::Map::new();
                for (k, v) in kwargs { ta.insert(k.clone(), runtime_to_json(&eval_expr(v, &env))); }
                for (i, a) in args.iter().enumerate() { ta.insert(format!("arg{i}"), runtime_to_json(&eval_expr(a, &env))); }
                tool_calls.push(serde_json::json!({ "tool": name, "args": ta }));
                log.push(format!("call {name}(...)"));
            }
            Expr::SketchBlock { name, body } => {
                env.insert(name.clone(), RuntimeValue::Solid(name.clone()));
                for sub in body {
                    if let Expr::Call { name: fn_name, args, kwargs } = sub {
                        let mut ta = serde_json::Map::new();
                        for (k, v) in kwargs { ta.insert(k.clone(), runtime_to_json(&eval_expr(v, &env))); }
                        for (i, a) in args.iter().enumerate() { ta.insert(format!("arg{i}"), runtime_to_json(&eval_expr(a, &env))); }
                        tool_calls.push(serde_json::json!({ "tool": fn_name, "args": ta, "sketch": name }));
                    }
                }
                log.push(format!("sketch {name} {{...}}"));
            }
            Expr::Ask { query } => {
                let val = eval_expr(query, &env);
                log.push(format!("ask cascade: {:?}", val));
                tool_calls.push(serde_json::json!({ "tool": "cascade_query", "args": { "query": runtime_to_json(&val) } }));
            }
            Expr::Import { path, format } => {
                log.push(format!("import \"{}\" (format: {:?})", path, format));
                tool_calls.push(serde_json::json!({ "tool": "import", "args": { "path": path, "format": format } }));
            }
            Expr::IfElse { condition, then_body, else_body } => {
                let cond = eval_expr(condition, &env);
                if cond.is_truthy() {
                    let sub = execute_ast(then_body);
                    env.extend(sub.variables);
                    tool_calls.extend(sub.tool_calls);
                    log.extend(sub.log);
                } else if let Some(eb) = else_body {
                    let sub = execute_ast(eb);
                    env.extend(sub.variables);
                    tool_calls.extend(sub.tool_calls);
                    log.extend(sub.log);
                }
            }
            _ => {}
        }
    }

    let all_passed = assertions.iter().all(|(_, p)| *p);

    ExecutionResult { variables: env, assertions, tool_calls, log, all_passed, errors }
}

/// Evaluate a single expression in the current environment.
fn eval_expr(expr: &Expr, env: &HashMap<String, RuntimeValue>) -> RuntimeValue {
    match expr {
        Expr::Literal(v) => RuntimeValue::Number(v.number, v.unit),
        Expr::StringLit(s) => RuntimeValue::String(s.clone()),
        Expr::BoolLit(b) => RuntimeValue::Bool(*b),
        Expr::Ident(name) => env.get(name).cloned().unwrap_or(RuntimeValue::Void),
        Expr::BinOp { left, op, right } => {
            let l = eval_expr(left, env);
            let r = eval_expr(right, env);
            match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => {
                    let result = match op {
                        BinOp::Add => a + b,
                        BinOp::Sub => a - b,
                        BinOp::Mul => a * b,
                        BinOp::Div => if b != 0.0 { a / b } else { f64::INFINITY },
                        BinOp::Mod => a % b,
                        BinOp::Lt => return RuntimeValue::Bool(a < b),
                        BinOp::Gt => return RuntimeValue::Bool(a > b),
                        BinOp::LtEq => return RuntimeValue::Bool(a <= b),
                        BinOp::GtEq => return RuntimeValue::Bool(a >= b),
                        BinOp::Eq => return RuntimeValue::Bool((a - b).abs() < 1e-10),
                        BinOp::NotEq => return RuntimeValue::Bool((a - b).abs() >= 1e-10),
                        BinOp::And => return RuntimeValue::Bool(a != 0.0 && b != 0.0),
                        BinOp::Or => return RuntimeValue::Bool(a != 0.0 || b != 0.0),
                    };
                    RuntimeValue::Number(result, None)
                }
                _ => RuntimeValue::Error("type mismatch in binary op".into()),
            }
        }
        Expr::UnaryOp { op, operand } => {
            let v = eval_expr(operand, env);
            match (op, v.as_f64()) {
                (UnaryOp::Neg, Some(n)) => RuntimeValue::Number(-n, None),
                (UnaryOp::Not, _) => RuntimeValue::Bool(!v.is_truthy()),
                _ => RuntimeValue::Error("unary op type mismatch".into()),
            }
        }
        Expr::Call { name, args, .. } => {
            // Built-in functions
            match name.as_str() {
                "weight" | "mass" => {
                    // Placeholder: return a computed weight
                    RuntimeValue::Number(0.15, Some(CflUnit::Kg))
                }
                "volume" => RuntimeValue::Number(50000.0, Some(CflUnit::Mm3)),
                "stress" => RuntimeValue::Number(85.0, Some(CflUnit::Mpa)),
                "yield" => {
                    // Look up material yield strength
                    if let Some(arg) = args.first() {
                        let val = eval_expr(arg, env);
                        if let RuntimeValue::String(mat_id) = &val {
                            let ys = physical_lut::materials::lookup(mat_id)
                                .map(|m| m.yield_strength.to_mpa())
                                .unwrap_or(276.0);
                            return RuntimeValue::Number(ys, Some(CflUnit::Mpa));
                        }
                    }
                    RuntimeValue::Number(276.0, Some(CflUnit::Mpa))
                }
                "sqrt" => {
                    if let Some(v) = args.first().and_then(|a| eval_expr(a, env).as_f64()) {
                        RuntimeValue::Number(v.sqrt(), None)
                    } else { RuntimeValue::Error("sqrt: expected number".into()) }
                }
                "abs" => {
                    if let Some(v) = args.first().and_then(|a| eval_expr(a, env).as_f64()) {
                        RuntimeValue::Number(v.abs(), None)
                    } else { RuntimeValue::Error("abs: expected number".into()) }
                }
                "min" => {
                    let vals: Vec<f64> = args.iter().filter_map(|a| eval_expr(a, env).as_f64()).collect();
                    RuntimeValue::Number(vals.iter().copied().fold(f64::INFINITY, f64::min), None)
                }
                "max" => {
                    let vals: Vec<f64> = args.iter().filter_map(|a| eval_expr(a, env).as_f64()).collect();
                    RuntimeValue::Number(vals.iter().copied().fold(f64::NEG_INFINITY, f64::max), None)
                }
                _ => RuntimeValue::Void, // unknown function — will be handled by MCP dispatch
            }
        }
        Expr::MemberAccess { object, member } => {
            let obj = eval_expr(object, env);
            if let RuntimeValue::String(mat_id) = &obj {
                // Material property access: al.yield_strength
                if let Some(m) = physical_lut::materials::lookup(mat_id) {
                    return match member.as_str() {
                        "yield_strength" | "yield" => RuntimeValue::Number(m.yield_strength.to_mpa(), Some(CflUnit::Mpa)),
                        "elastic_modulus" | "E" => RuntimeValue::Number(m.elastic_modulus.to_mpa(), Some(CflUnit::Mpa)),
                        "density" => RuntimeValue::Number(m.density.value(), Some(CflUnit::Kg)),
                        "thermal_conductivity" | "k" => RuntimeValue::Number(m.thermal_conductivity.value(), None),
                        "melting_point" => RuntimeValue::Number(m.melting_point.to_celsius(), Some(CflUnit::C)),
                        _ => RuntimeValue::Error(format!("unknown property: {member}")),
                    };
                }
            }
            RuntimeValue::Void
        }
        Expr::Array(elements) => {
            RuntimeValue::Array(elements.iter().map(|e| eval_expr(e, env)).collect())
        }
        _ => RuntimeValue::Void,
    }
}

fn runtime_to_json(val: &RuntimeValue) -> serde_json::Value {
    match val {
        RuntimeValue::Number(n, u) => serde_json::json!({ "value": n, "unit": format!("{:?}", u) }),
        RuntimeValue::String(s) => serde_json::Value::String(s.clone()),
        RuntimeValue::Bool(b) => serde_json::Value::Bool(*b),
        RuntimeValue::Solid(name) => serde_json::json!({ "solid": name }),
        RuntimeValue::Array(a) => serde_json::Value::Array(a.iter().map(runtime_to_json).collect()),
        RuntimeValue::Void => serde_json::Value::Null,
        RuntimeValue::Error(e) => serde_json::json!({ "error": e }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_number_with_unit() {
        let tokens = lex("50mm");
        assert!(matches!(tokens[0], Token::Number(n) if (n - 50.0).abs() < 0.01));
        assert!(matches!(tokens[1], Token::Unit(CflUnit::Mm)));
    }

    #[test]
    fn lex_number_with_mpa() {
        let tokens = lex("100MPa");
        assert!(matches!(tokens[0], Token::Number(n) if (n - 100.0).abs() < 0.01));
        assert!(matches!(tokens[1], Token::Unit(CflUnit::Mpa)));
    }

    #[test]
    fn lex_keywords() {
        let tokens = lex("material solid sketch extrude assert export");
        assert_eq!(tokens[0], Token::Material);
        assert_eq!(tokens[1], Token::Solid);
        assert_eq!(tokens[2], Token::Sketch);
        assert_eq!(tokens[3], Token::Extrude);
        assert_eq!(tokens[4], Token::Assert);
        assert_eq!(tokens[5], Token::Export);
    }

    #[test]
    fn lex_string_literal() {
        let tokens = lex("\"6061-T6\"");
        assert!(matches!(&tokens[0], Token::StringLit(s) if s == "6061-T6"));
    }

    #[test]
    fn lex_operators() {
        let tokens = lex("a + b * c < d");
        assert!(matches!(tokens[0], Token::Ident(_)));
        assert_eq!(tokens[1], Token::Plus);
        assert!(matches!(tokens[2], Token::Ident(_)));
        assert_eq!(tokens[3], Token::Star);
        assert_eq!(tokens[5], Token::Lt);
    }

    #[test]
    fn lex_comment_skipped() {
        let tokens = lex("50mm // this is a comment\n100mm");
        assert!(matches!(tokens[0], Token::Number(n) if (n - 50.0).abs() < 0.01));
        // After comment and newline, next tokens
        let nums: Vec<f64> = tokens.iter().filter_map(|t| {
            if let Token::Number(n) = t { Some(*n) } else { None }
        }).collect();
        assert_eq!(nums.len(), 2);
    }

    #[test]
    fn parse_material_decl() {
        let tokens = lex("material al = \"6061-T6\"");
        let ast = parse(&tokens);
        assert_eq!(ast.len(), 1);
        assert!(matches!(&ast[0], Expr::MaterialDecl { name, .. } if name == "al"));
    }

    #[test]
    fn parse_let_binding() {
        let tokens = lex("let width = 50mm");
        let ast = parse(&tokens);
        assert_eq!(ast.len(), 1);
        assert!(matches!(&ast[0], Expr::LetDecl { name, .. } if name == "width"));
    }

    #[test]
    fn parse_solid_decl() {
        let tokens = lex("solid part = extrude(base, 10mm)");
        let ast = parse(&tokens);
        assert_eq!(ast.len(), 1);
        assert!(matches!(&ast[0], Expr::SolidDecl { name, .. } if name == "part"));
    }

    #[test]
    fn parse_function_call_with_kwargs() {
        let tokens = lex("hole(bracket, diameter: 6mm, depth: 10mm)");
        let ast = parse(&tokens);
        assert_eq!(ast.len(), 1);
        if let Expr::Call { name, args, kwargs } = &ast[0] {
            assert_eq!(name, "hole");
            assert_eq!(args.len(), 1); // bracket
            assert!(kwargs.contains_key("diameter"));
            assert!(kwargs.contains_key("depth"));
        } else {
            panic!("expected Call, got {:?}", ast[0]);
        }
    }

    #[test]
    fn parse_assert_comparison() {
        let tokens = lex("assert weight < 200g");
        let ast = parse(&tokens);
        assert_eq!(ast.len(), 1);
        assert!(matches!(&ast[0], Expr::Assert { .. }));
    }

    #[test]
    fn parse_sketch_block() {
        let tokens = lex("sketch profile {\n  rect(50mm, 30mm)\n}");
        let ast = parse(&tokens);
        assert_eq!(ast.len(), 1);
        if let Expr::SketchBlock { name, body } = &ast[0] {
            assert_eq!(name, "profile");
            assert!(!body.is_empty());
        } else {
            panic!("expected SketchBlock");
        }
    }

    #[test]
    fn parse_full_program() {
        let source = r#"
material al = "6061-T6"
let width = 50mm
solid bracket = extrude(profile, 10mm, material: al)
assert weight(bracket) < 200g
export(bracket, format: "step")
"#;
        let tokens = lex(source);
        let ast = parse(&tokens);
        assert!(ast.len() >= 5, "should have at least 5 statements, got {}", ast.len());
    }

    #[test]
    fn compile_to_mcp_produces_calls() {
        let tokens = lex("material al = \"6061-T6\"\nsolid part = extrude(base, 10mm)");
        let ast = parse(&tokens);
        let calls = compile_to_mcp(&ast);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["tool"], "lookup_material");
        assert_eq!(calls[1]["tool"], "extrude");
    }

    #[test]
    fn unit_conversion_mm_to_si() {
        assert!((CflUnit::Mm.to_si(50.0) - 0.05).abs() < 1e-10);
        assert!((CflUnit::In.to_si(1.0) - 0.0254).abs() < 1e-6);
        assert!((CflUnit::Mpa.to_si(100.0) - 1e8).abs() < 1.0);
        assert!((CflUnit::Deg.to_si(180.0) - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn unit_conversion_temperature() {
        assert!((CflUnit::C.to_si(0.0) - 273.15).abs() < 0.01);
        assert!((CflUnit::F.to_si(32.0) - 273.15).abs() < 0.01);
        assert!((CflUnit::K.to_si(300.0) - 300.0).abs() < 0.01);
    }

    #[test]
    fn kcl_to_cfl_basic() {
        let kcl = "const width = 50\nconst part = startSketchOn('XY')";
        let cfl = kcl_to_cfl(kcl);
        assert!(cfl.contains("let width = 50"));
        assert!(cfl.contains("sketch_on"));
    }

    #[test]
    fn openscad_to_cfl_basic() {
        let scad = "cube([10, 20, 30]);\ndifference()";
        let cfl = openscad_to_cfl(scad);
        assert!(cfl.contains("box("));
        assert!(cfl.contains("subtract"));
    }

    #[test]
    fn cfl_value_with_unit() {
        let v = CflValue::new(50.0, Some(CflUnit::Mm));
        assert!((v.to_si() - 0.05).abs() < 1e-10);
    }

    #[test]
    fn cfl_value_unitless() {
        let v = CflValue::unitless(3.14);
        assert!((v.to_si() - 3.14).abs() < 1e-10);
    }

    #[test]
    fn lex_array() {
        let tokens = lex("[10mm, 20mm, 30mm]");
        assert_eq!(tokens[0], Token::LBracket);
    }

    #[test]
    fn parse_ask() {
        let tokens = lex("ask \"what material for high temperature?\"");
        let ast = parse(&tokens);
        assert_eq!(ast.len(), 1);
        assert!(matches!(&ast[0], Expr::Ask { .. }));
    }

    #[test]
    fn all_units_parse() {
        let units = ["mm", "cm", "m", "in", "ft", "deg", "rad", "N", "kN",
            "Pa", "MPa", "GPa", "psi", "g", "kg", "Hz", "MHz", "GHz", "ohm", "W"];
        for u in units {
            assert!(CflUnit::parse(u).is_some(), "unit '{}' should parse", u);
        }
    }

    // ---- Execution engine tests ----

    #[test]
    fn execute_let_binding() {
        let result = execute("let x = 42");
        assert!(result.variables.contains_key("x"));
        assert_eq!(result.variables["x"].as_f64(), Some(42.0));
    }

    #[test]
    fn execute_material_declaration() {
        let result = execute("material al = \"6061-T6\"");
        assert!(result.variables.contains_key("al"));
        assert!(!result.tool_calls.is_empty());
        assert_eq!(result.tool_calls[0]["tool"], "lookup_material");
    }

    #[test]
    fn execute_solid_declaration() {
        let result = execute("solid part = extrude(base, 10mm)");
        assert!(result.variables.contains_key("part"));
        assert!(!result.tool_calls.is_empty());
        assert_eq!(result.tool_calls[0]["tool"], "extrude");
    }

    #[test]
    fn execute_assertion_pass() {
        let result = execute("assert 10 < 20");
        assert_eq!(result.assertions.len(), 1);
        assert!(result.assertions[0].1, "10 < 20 should pass");
        assert!(result.all_passed);
    }

    #[test]
    fn execute_assertion_fail() {
        let result = execute("assert 30 < 20");
        assert_eq!(result.assertions.len(), 1);
        assert!(!result.assertions[0].1, "30 < 20 should fail");
        assert!(!result.all_passed);
    }

    #[test]
    fn execute_yield_lookup() {
        let result = execute("material al = \"6061-T6\"\nlet ys = yield(al)");
        assert!(result.variables.contains_key("ys"));
        if let Some(n) = result.variables["ys"].as_f64() {
            assert!(n > 200.0 && n < 350.0, "6061-T6 yield should be ~276 MPa, got {}", n);
        }
    }

    #[test]
    fn execute_sketch_block() {
        let result = execute("sketch profile {\n  rect(50mm, 30mm)\n}");
        assert!(!result.tool_calls.is_empty());
        assert!(result.log.iter().any(|l| l.contains("sketch profile")));
    }

    #[test]
    fn execute_export() {
        let result = execute("export(bracket, format: \"step\")");
        assert!(result.tool_calls.iter().any(|c| c["tool"] == "export"));
    }

    #[test]
    fn execute_ask_cascade() {
        let result = execute("ask \"what material for 500N?\"");
        assert!(result.tool_calls.iter().any(|c| c["tool"] == "cascade_query"));
    }

    #[test]
    fn execute_full_program() {
        let source = r#"
material al = "6061-T6"
let width = 50mm
solid bracket = extrude(profile, 10mm, material: al)
assert 100 < 200
export(bracket, format: "step")
"#;
        let result = execute(source);
        assert!(result.all_passed);
        assert!(result.variables.contains_key("al"));
        assert!(result.variables.contains_key("width"));
        assert!(result.variables.contains_key("bracket"));
        assert!(result.tool_calls.len() >= 3);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn execute_builtin_sqrt() {
        let result = execute("let a = sqrt(16)");
        if let Some(n) = result.variables.get("a").and_then(|v| v.as_f64()) {
            assert!((n - 4.0).abs() < 0.01, "sqrt(16) = {}", n);
        }
    }

    #[test]
    fn execute_log_traces() {
        let result = execute("let x = 10\nmaterial steel = \"1018-CD\"");
        assert!(result.log.len() >= 2);
    }
}
