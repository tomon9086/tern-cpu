//! アセンブラ(M2)。文法は docs/isa.md 5 章。2 パスでラベルを解決する。

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::cpu::{encode, Op, ADDR_MAX};
use crate::trit::{Trit, Word9};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsmError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for AsmError {}

fn err<T>(line: usize, msg: impl Into<String>) -> Result<T, AsmError> {
    Err(AsmError {
        line,
        msg: msg.into(),
    })
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    pub image: BTreeMap<i16, Word9>,
    pub labels: HashMap<String, i16>,
    /// アドレス → ソースの行番号
    pub source_lines: BTreeMap<i16, usize>,
}

enum Kind {
    Insn(Op),
    Word,
}

struct Item {
    line: usize,
    addr: i16,
    kind: Kind,
    operand: Option<String>,
}

fn is_label(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// 10 進数(`-12`、`+3`)または三進リテラル(`0t+0-`)。
fn parse_number(tok: &str) -> Option<i64> {
    let tok = tok.trim();
    if let Some(body) = tok.strip_prefix("0t").or_else(|| tok.strip_prefix("0T")) {
        if body.is_empty() {
            return None;
        }
        let mut v: i64 = 0;
        for c in body.chars().filter(|&c| c != '_') {
            v = v.checked_mul(3)?.checked_add(Trit::from_char(c)? as i64)?;
        }
        return Some(v);
    }
    let digits = tok.strip_prefix(['+', '-']).unwrap_or(tok);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    tok.parse().ok()
}

/// 数値、ラベル、`ラベル ± 数値`
fn eval(expr: &str, labels: &HashMap<String, i16>, line: usize) -> Result<i64, AsmError> {
    let expr = expr.trim();
    if let Some(n) = parse_number(expr) {
        return Ok(n);
    }
    let (name, rest) = match expr.find(['+', '-']) {
        Some(i) => (
            expr[..i].trim(),
            Some((&expr[i..i + 1], expr[i + 1..].trim())),
        ),
        None => (expr, None),
    };
    if !is_label(name) {
        return err(line, format!("bad operand {expr:?}"));
    }
    let Some(&base) = labels.get(name) else {
        return err(line, format!("undefined label {name:?}"));
    };
    let mut v = base as i64;
    if let Some((sign, off)) = rest {
        let Some(k) = parse_number(off).filter(|_| !off.starts_with(['+', '-'])) else {
            return err(line, format!("bad offset in {expr:?}"));
        };
        v = if sign == "+" { v + k } else { v - k };
    }
    Ok(v)
}

pub fn assemble(source: &str) -> Result<Program, AsmError> {
    let mut prog = Program::default();
    let mut items: Vec<Item> = Vec::new();
    let mut addr: i64 = 0;

    // パス 1: ラベルとアドレスの割り当て
    for (idx, raw) in source.lines().enumerate() {
        let line = idx + 1;
        let mut text = raw.split(';').next().unwrap_or("").trim();
        while let Some(i) = text.find(':') {
            let label = text[..i].trim();
            if !is_label(label) {
                return err(line, format!("bad label {label:?}"));
            }
            if prog.labels.contains_key(label) {
                return err(line, format!("duplicate label {label:?}"));
            }
            prog.labels.insert(label.to_string(), addr as i16);
            text = text[i + 1..].trim();
        }
        if text.is_empty() {
            continue;
        }
        let (mnem, arg) = match text.split_once(char::is_whitespace) {
            Some((m, a)) => (m, Some(a.trim().to_string())),
            None => (text, None),
        };

        if mnem.eq_ignore_ascii_case(".org") {
            let Some(n) = arg.as_deref().and_then(parse_number) else {
                return err(line, ".org needs a numeric address");
            };
            if n.abs() > ADDR_MAX as i64 {
                return err(line, format!("bad .org address {n}"));
            }
            addr = n;
            continue;
        }
        if addr > ADDR_MAX as i64 {
            return err(line, "program exceeds memory");
        }
        let kind = if mnem.eq_ignore_ascii_case(".word") {
            if arg.is_none() {
                return err(line, ".word needs a value");
            }
            Kind::Word
        } else {
            let Some(op) = Op::from_name(mnem) else {
                return err(line, format!("unknown mnemonic {mnem:?}"));
            };
            if arg.is_none() && op.takes_operand() {
                return err(line, format!("{} needs an operand", op.name()));
            }
            Kind::Insn(op)
        };
        items.push(Item {
            line,
            addr: addr as i16,
            kind,
            operand: arg,
        });
        addr += 1;
    }

    // パス 2: 符号化
    for it in items {
        if prog.image.contains_key(&it.addr) {
            return err(it.line, format!("address {} assigned twice", it.addr));
        }
        let v = match &it.operand {
            Some(e) => eval(e, &prog.labels, it.line)?,
            None => 0,
        };
        let w = match it.kind {
            Kind::Word => match Word9::from_i64(v) {
                Ok(w) => w,
                Err(e) => return err(it.line, e.to_string()),
            },
            Kind::Insn(op) => {
                if v.abs() > ADDR_MAX as i64 {
                    return err(it.line, format!("operand {v} out of range ±{ADDR_MAX}"));
                }
                encode(op as i8, v as i16).expect("checked range")
            }
        };
        prog.image.insert(it.addr, w);
        prog.source_lines.insert(it.addr, it.line);
    }
    Ok(prog)
}
