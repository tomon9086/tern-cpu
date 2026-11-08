//! 平衡三進 CPU エミュレータ。
//!
//! - [`trit`] — トリット・ワード演算(M1, docs/balanced-ternary.md)
//! - [`cpu`] — ISA レベル エミュレータ(M2, docs/isa.md)
//! - [`asm`] — アセンブラ(M2)

pub mod asm;
pub mod cpu;
pub mod trit;

pub use trit::{add_trit, Trit, Word, Word3, Word6, Word9};
