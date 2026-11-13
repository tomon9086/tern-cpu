# tern

3進 CPU をつくる。平衡三進法(各桁 -1 / 0 / +1)の CPU を、
エミュレータ → ゲートレベル → SPICE → ブレッドボード → PCB の順に段階的に作るプロジェクト。

## 現状

| マイルストーン | 内容 | 状態 |
|---|---|---|
| M0 | リポジトリ骨組み・平衡三進の仕様書 | 済 |
| M1 | 数値表現ライブラリ(`emu/src/trit.rs`) | 済 |
| M2 | ISA レベルエミュレータ・アセンブラ(`emu/src/cpu.rs`, `emu/src/asm.rs`) | 済(ISA v0) |
| M3 | ゲートレベルエミュレータ・部品数集計(`emu/src/gates.rs`) | 初版済(結果は [`docs/gates.md`](docs/gates.md) 6 章) |
| M4 | SPICE による回路検証(`spice/`) | 着手(STI の 2 方式を比較、[`docs/spice.md`](docs/spice.md)) |
| M5〜 | ブレッドボード / 実機 | 未着手 |

## ビルドとテスト

エミュレータは Rust(標準ライブラリのみ、外部クレートなし)。言語選定の経緯は ADR 0001 / 0004。

```bash
cd emu
```

```bash
cargo test
```

9 トリット加算の全網羅テスト(約 3.9 億通り。4 コアでワード単位は約 9 秒、ゲートレベルは約 4 分)は重いので通常は除外している:

```bash
cargo test --release -- --ignored
```

## プログラムの実行

ISA の仕様は [`docs/isa.md`](docs/isa.md)。`emu/` で:

```bash
cargo run --bin t3run -- ../programs/multiply.t3asm
```

```bash
cargo run --bin t3run -- ../programs/countdown.t3asm --trace
```

`--list` でアセンブル結果(アドレス・命令語・逆アセンブル)を表示する。
`--alu gate` を付けると、ADD をゲートレベルの 9 トリット加算器(M3 の標準構成)で計算する。

### プログラムを書く

`programs/` に `.t3asm` ファイルを置いて実行する。命令と文法は [`docs/isa.md`](docs/isa.md)。

```
; 10 + (-3) を出力する
        LDA  x
        ADD  y
        OUT
        HALT
x:      .word 10
y:      .word -3     ; 三進リテラルなら 0t-0(= -3)
```

- 命令: `HALT` `LDA a` `STA a` `LDI n` `ADD a` `NEG` `JMP a` `BR3 a` `OUT`
- 条件分岐は `BR3 a`(ACC が負なら a-1、0 なら a、正なら a+1 へ)。分岐表の書き方は `countdown.t3asm` を参照
- サンプル: `sum3x5`(15)、`countdown`(5〜1)、`multiply`(7 × -12)、`fibonacci`(15 項)

ゲートレベル回路の部品数・段数の集計:

```bash
cargo run --release --bin gatestat -- --breakdown
```

## ライブラリの使い方

```rust
use tern_emu::{Trit, Word3};

let a = Word3::from_i64(8).unwrap();       // +0-
let b = Word3::parse("+-0").unwrap();      // 9 - 3 = 6
let (r, cout) = a.add_carry(b, Trit::Z);  // 14 は収まらない → r = --- (-13), cout = +
println!("{} {:?} {}", -a, a.sign(), a << 1);
```

## ディレクトリ

```
docs/        仕様(balanced-ternary.md, isa.md, gates.md, glossary.md)と設計判断(decisions/)
emu/         エミュレータ(Cargo パッケージ。src/: trit / cpu / asm / gates / bin/{t3run,gatestat}、tests/)
programs/    サンプルプログラム(sum3x5, countdown, multiply, fibonacci)
spice/       SPICE の部品モデル・ゲート回路・評価スクリプト(M4〜。python3 spice/sim.py)
hardware/    KiCad プロジェクト(M5〜)
```
