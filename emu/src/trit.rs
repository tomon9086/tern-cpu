//! 平衡三進のトリット・ワード演算(M1)。
//!
//! 仕様は docs/balanced-ternary.md。ワードは LSB first(ADR 0002)。

use std::fmt;
use std::ops::{Add, Neg, Shl, Shr, Sub};

/// 1 トリット(-1 / 0 / +1)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i8)]
pub enum Trit {
    N = -1,
    Z = 0,
    P = 1,
}

impl Trit {
    /// -1, 0, +1 の順。
    pub const ALL: [Trit; 3] = [Trit::N, Trit::Z, Trit::P];

    pub const fn from_i8(v: i8) -> Option<Trit> {
        match v {
            -1 => Some(Trit::N),
            0 => Some(Trit::Z),
            1 => Some(Trit::P),
            _ => None,
        }
    }

    pub const fn value(self) -> i8 {
        self as i8
    }

    /// 標準インバータ: -x
    pub const fn sti(self) -> Trit {
        match self {
            Trit::N => Trit::P,
            Trit::Z => Trit::Z,
            Trit::P => Trit::N,
        }
    }

    /// 負インバータ: x が - なら +、それ以外は -
    pub const fn nti(self) -> Trit {
        match self {
            Trit::N => Trit::P,
            _ => Trit::N,
        }
    }

    /// 正インバータ: x が + なら -、それ以外は +
    pub const fn pti(self) -> Trit {
        match self {
            Trit::P => Trit::N,
            _ => Trit::P,
        }
    }

    pub fn min3(self, other: Trit) -> Trit {
        self.min(other)
    }

    pub fn max3(self, other: Trit) -> Trit {
        self.max(other)
    }

    pub const fn to_char(self) -> char {
        match self {
            Trit::N => '-',
            Trit::Z => '0',
            Trit::P => '+',
        }
    }

    /// `-`/`T`/`t` → -1、`0` → 0、`+`/`1` → +1
    pub const fn from_char(c: char) -> Option<Trit> {
        match c {
            '-' | 'T' | 't' => Some(Trit::N),
            '0' => Some(Trit::Z),
            '+' | '1' => Some(Trit::P),
            _ => None,
        }
    }
}

const fn trit(v: i8) -> Trit {
    match v {
        -1 => Trit::N,
        0 => Trit::Z,
        1 => Trit::P,
        _ => panic!("not a trit"),
    }
}

/// 1 トリット全加算器。`a + b + cin == sum + 3 * carry` となる `(sum, carry)` を返す。
pub const fn add_trit(a: Trit, b: Trit, cin: Trit) -> (Trit, Trit) {
    let s = a as i8 + b as i8 + cin as i8; // -3..=3
    let sum = (s + 1).rem_euclid(3) - 1;
    (trit(sum), trit((s - sum) / 3))
}

/// トリット列(LSB first)の値。
pub fn trits_value(trits: &[Trit]) -> i64 {
    trits.iter().rev().fold(0, |v, &t| v * 3 + t as i64)
}

/// `(3^width - 1) / 2`。width トリットで表せる最大値。
pub const fn max_range(width: usize) -> i64 {
    (3i64.pow(width as u32) - 1) / 2
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overflow {
    pub value: i64,
    pub width: usize,
}

impl fmt::Display for Overflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} does not fit in {} trits (range ±{})",
            self.value,
            self.width,
            max_range(self.width)
        )
    }
}

impl std::error::Error for Overflow {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    BadChar(char),
    TooLong { len: usize, width: usize },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty trit string"),
            ParseError::BadChar(c) => write!(f, "invalid trit character {c:?}"),
            ParseError::TooLong { len, width } => {
                write!(f, "{len} trits do not fit in a {width}-trit word")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// N トリットのワード(不変値)。`self.0[i]` が 3^i の桁(LSB first)。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Word<const N: usize>(pub [Trit; N]);

/// CPU のワード(命令・データ)
pub type Word9 = Word<9>;
/// アドレス幅
pub type Word6 = Word<6>;
/// 実機 ALU 試作の幅
pub type Word3 = Word<3>;
pub type Word4 = Word<4>;
pub type Word5 = Word<5>;

impl<const N: usize> Word<N> {
    /// 表せる最大値。最小値は `-MAX`。
    pub const MAX: i64 = max_range(N);
    /// 表せる値の個数 3^N。
    pub const MODULUS: i64 = 3i64.pow(N as u32);
    pub const ZERO: Self = Word([Trit::Z; N]);

    /// 整数から生成。範囲外は `Overflow`。
    pub fn from_i64(value: i64) -> Result<Self, Overflow> {
        let (w, rest) = Self::split_i64(value);
        if rest == 0 {
            Ok(w)
        } else {
            Err(Overflow { value, width: N })
        }
    }

    /// 整数から生成。3^N を法として平衡範囲にラップする。
    pub fn from_i64_wrapping(value: i64) -> Self {
        Self::split_i64(value).0
    }

    fn split_i64(mut v: i64) -> (Self, i64) {
        let mut t = [Trit::Z; N];
        for slot in t.iter_mut() {
            let mut r = v.rem_euclid(3);
            if r == 2 {
                r = -1;
            }
            *slot = trit(r as i8);
            v = (v - r) / 3;
        }
        (Word(t), v)
    }

    /// LSB first のスライスから生成(長さが N でなければ `None`)。
    pub fn from_slice(trits: &[Trit]) -> Option<Self> {
        Some(Word(trits.try_into().ok()?))
    }

    /// MSB first の文字列(`+0-`、`10T` など)から生成。短ければ上位を 0 で埋める。
    /// `_` は区切りとして無視する。
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let chars: Vec<char> = text.trim().chars().filter(|&c| c != '_').collect();
        if chars.is_empty() {
            return Err(ParseError::Empty);
        }
        if chars.len() > N {
            return Err(ParseError::TooLong {
                len: chars.len(),
                width: N,
            });
        }
        let mut t = [Trit::Z; N];
        for (i, &c) in chars.iter().rev().enumerate() {
            t[i] = Trit::from_char(c).ok_or(ParseError::BadChar(c))?;
        }
        Ok(Word(t))
    }

    pub fn trits(&self) -> &[Trit; N] {
        &self.0
    }

    pub fn to_i64(self) -> i64 {
        trits_value(&self.0)
    }

    /// 0 でない最上位トリット(全 0 なら 0)。BR3 の分岐判定に使う。
    pub fn sign(self) -> Trit {
        self.0
            .iter()
            .rev()
            .copied()
            .find(|&t| t != Trit::Z)
            .unwrap_or(Trit::Z)
    }

    /// リップルキャリー加算。`self + rhs + cin == 結果 + cout * 3^N`。
    pub fn add_carry(self, rhs: Self, cin: Trit) -> (Self, Trit) {
        let mut out = [Trit::Z; N];
        let mut c = cin;
        for (o, (&a, &b)) in out.iter_mut().zip(self.0.iter().zip(&rhs.0)) {
            (*o, c) = add_trit(a, b, c);
        }
        (Word(out), c)
    }

    /// 符号反転(各トリットを STI)。オーバーフローは起きない。
    pub fn negate(self) -> Self {
        Word(self.0.map(Trit::sti))
    }
}

impl<const N: usize> Default for Word<N> {
    fn default() -> Self {
        Self::ZERO
    }
}

/// ラップする加算(キャリーは捨てる)。キャリーが必要なら `add_carry` を使う。
impl<const N: usize> Add for Word<N> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.add_carry(rhs, Trit::Z).0
    }
}

impl<const N: usize> Sub for Word<N> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.add_carry(rhs.negate(), Trit::Z).0
    }
}

/// 左シフト(×3^k、上位はあふれて捨てる)。
impl<const N: usize> Shl<usize> for Word<N> {
    type Output = Self;
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn shl(self, k: usize) -> Self {
        let k = k.min(N);
        let mut t = [Trit::Z; N];
        t[k..].copy_from_slice(&self.0[..N - k]);
        Word(t)
    }
}

/// 右シフト(3^k で割って最も近い整数に丸める)。
impl<const N: usize> Shr<usize> for Word<N> {
    type Output = Self;
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn shr(self, k: usize) -> Self {
        let k = k.min(N);
        let mut t = [Trit::Z; N];
        t[..N - k].copy_from_slice(&self.0[k..]);
        Word(t)
    }
}

impl<const N: usize> Neg for Word<N> {
    type Output = Self;
    fn neg(self) -> Self {
        self.negate()
    }
}

/// MSB first(例: `+0-`)
impl<const N: usize> fmt::Display for Word<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: String = self.0.iter().rev().map(|t| t.to_char()).collect();
        f.pad(&s)
    }
}

impl<const N: usize> fmt::Debug for Word<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Word<{N}>({self} = {})", self.to_i64())
    }
}
