# tern

3進 CPU をつくる。平衡三進法(各桁 -1 / 0 / +1)の CPU を、
エミュレータ → ゲートレベル → SPICE → ブレッドボード → PCB の順に段階的に作るプロジェクト。

## 現状

| マイルストーン | 内容 | 状態 |
|---|---|---|
| M0 | リポジトリ骨組み・平衡三進の仕様書 | 済 |
| M1 | 数値表現ライブラリ(`emu/trit.py`) | 済 |
| M2 | ISA レベルエミュレータ・アセンブラ | 未着手(ISA 未決) |
| M3 | ゲートレベルエミュレータ・トランジスタ数集計 | 未着手 |
| M4〜 | SPICE / ブレッドボード / 実機 | 未着手 |

## テスト

依存なし(Python 3.10 以上、標準ライブラリのみ)。

```bash
python3 -m unittest discover -s tests -t .
```

## 使い方(M1)

```python
from emu.trit import Word

a = Word.from_int(8, 3)     # Word('+0-' = 8)
b = Word.parse("+-0")        # 9 - 3 = 6
r, cout = a.add(b)           # 14 は 3 トリットに収まらない → r = -13, cout = +1
print(-a, a.sign(), a.shl(1))
```

## ディレクトリ

```
docs/        仕様(balanced-ternary.md, 今後 isa.md, gates.md)と設計判断(decisions/)
emu/         エミュレータ(trit.py: M1)
tests/       テスト
programs/    サンプルプログラム(M2〜)
spice/       SPICE ネットリスト(M4〜)
hardware/    KiCad プロジェクト(M5〜)
```
