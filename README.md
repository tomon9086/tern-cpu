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
| M4〜 | SPICE / ブレッドボード / 実機 | 未着手 |

## ビルドとテスト

エミュレータは Rust(標準ライブラリのみ、外部クレートなし)。言語選定の経緯は ADR 0001 / 0004。

```bash
cd emu
```

```bash
cargo test
```

9 トリット加算の全網羅テスト(約 3.9 億通り。ワード単位は 4 コアで約 9 秒、ゲートレベルはさらに長い)は重いので通常は除外している:

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
programs/    サンプルプログラム(sum3x5, countdown, multiply)
spice/       SPICE ネットリスト(M4〜)
hardware/    KiCad プロジェクト(M5〜)
```
