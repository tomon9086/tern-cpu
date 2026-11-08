# 用語集

| 用語 | 意味 |
|---|---|
| トリット(trit) | 3 進法の 1 桁。平衡三進では -1, 0, +1 |
| トライト(tryte) | トリットをまとめた単位。2 進のバイトに相当。Setun-70 では 6 トリット(729 通り)。本プロジェクトでは単位として使っていない(ADR 0005) |
| ワード(word) | CPU が一度に扱うデータの単位(レジスタ・ALU の幅、メモリ 1 番地の量)。ISA v0 では 9 トリット |
| 平衡三進法 | 各桁が -1/0/+1 の 3 進法。符号が不要で、反転が簡単 |
| STI / NTI / PTI | 3 値インバータの種類(標準 / 負 / 正)。定義は balanced-ternary.md 7 章 |
| MIN / MAX | 3 値の AND / OR に相当する演算 |
| ALU | 計算ユニット。今回はまず加算器が中心 |
| ACC | アキュムレータ。ISA v0 の唯一の演算用レジスタ |
| Setun | 1958 年にモスクワ国立大学で作られた平衡三進コンピュータ |
| Setun-70 | Setun の後継(1970 年)。6 トリットのトライト単位、2 スタック方式 |
| ハイブリッド構成 | 計算は 3 進回路、メモリ・制御は 2 進マイコンに任せる構成 |

## 出典メモ

- Setun-70 のトライト = 6 トリット: [Wikipedia: Setun-70](https://en.wikipedia.org/wiki/Setun-70)、
  [Russian Virtual Computer Museum](https://computer-museum.ru/english/setun.htm)、
  [HandWiki: Setun-70](https://handwiki.org/wiki/Setun-70)
- Setun-70 の命令形式の詳細は未確認。一次資料候補: Brusentsov, Zhogolev, Maslov,
  "General Characteristics of the Small Digital Machine 'Setun 70'" (1970)
