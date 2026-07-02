<div align="center">
  <img src="icon.png" width="128" alt="karukan" />
  <h1>Karukan</h1>
  <p>Linux・macOS向け日本語入力システム — ニューラルかな漢字変換エンジン</p>

  [![CI (engine)](https://github.com/togatoga/karukan/actions/workflows/karukan-engine-ci.yml/badge.svg)](https://github.com/togatoga/karukan/actions/workflows/karukan-engine-ci.yml)
  [![CI (im)](https://github.com/togatoga/karukan/actions/workflows/karukan-im-ci.yml/badge.svg)](https://github.com/togatoga/karukan/actions/workflows/karukan-im-ci.yml)
  [![CI (fcitx5)](https://github.com/togatoga/karukan/actions/workflows/karukan-fcitx5-ci.yml/badge.svg)](https://github.com/togatoga/karukan/actions/workflows/karukan-fcitx5-ci.yml)
  [![CI (macos)](https://github.com/togatoga/karukan/actions/workflows/karukan-macos-ci.yml/badge.svg)](https://github.com/togatoga/karukan/actions/workflows/karukan-macos-ci.yml)
  [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
</div>

<div align="center">
  <img src="images/demo.gif" width="800" alt="karukan demo" />
</div>

> これは kazuph による Karukan フォークで、Google IME 風UXに改変済みです。入力中は勝手に変換せず、Tabで予測、Spaceで通常変換を開始する操作感をデフォルトにしています。

## kazuph フォークの Google IME 風UX

このフォークは「ユーザーが操作するまで、ひらがなの Composing を維持する」ことを優先します。ライブ変換はデフォルトOFFで、必要な場合は `config.toml` の `conversion.live_conversion = true` または `Ctrl+Shift+L` でONに戻せます。

| 操作 | このフォークの動作 | 本家 Karukan との差分 |
|------|------------------|----------------------|
| 文字入力中 | ひらがなのまま。候補窓は学習履歴・ユーザー辞書の予測だけを未選択で表示 | 入力ごとのモデル推論候補を混ぜず、1行目の偽ハイライトも出さない |
| Tab / ↓ | 表示中の予測候補をそのまま選択。予測0件なら何もしない | 旧Tabの学習スキップ変換は `conversion.tab_skips_learning = true` で復活 |
| Shift+Tab / ↑ | 表示中の予測候補を逆方向に選択 | 予測リストを再計算しない |
| Ctrl+N / Ctrl+P | 予測選択中に次/前の候補へ移動 | Emacs風移動を予測候補にも対応 |
| 数字1-9 | 表示中の予測候補を即確定 | Tabを押さずに候補番号で確定 |
| Enter | 未選択ならひらがなをそのまま確定。選択中なら候補を確定 | ひらがな確定は学習キャッシュに記録しない |
| Space | 読みと同じ長さの通常変換を開始 | 学習キャッシュの前方一致予測を混ぜない |
| Esc | 予測選択中はひらがな Composing に戻る | 選択状態と表示が一致 |

```mermaid
stateDiagram-v2
    direction LR
    Composing: Composing\nひらがな維持\n予測窓は未選択
    Predict: 予測選択\nTab/↓/Ctrl+N/P/数字
    Conversion: 通常変換\nSpace
    Committed: 確定

    Composing --> Predict: Tab / ↓\n予測がある
    Composing --> Conversion: Space
    Composing --> Committed: Enter\n学習記録なし
    Predict --> Predict: Tab / ↓ / Shift+Tab / ↑\nCtrl+N / Ctrl+P
    Predict --> Committed: Enter / 数字1-9\n学習記録あり
    Predict --> Composing: Esc
    Conversion --> Committed: Enter / 数字1-9\n学習記録あり
    Conversion --> Composing: Esc
```

## プロジェクト構成

| クレート | 説明 |
|---------|------|
| [karukan-fcitx5](karukan-fcitx5/) | Linux向けIMEフロントエンド — fcitx5アドオン + C FFI |
| [karukan-macos](karukan-macos/) | macOS向けIMEフロントエンド — Swift/InputMethodKit |
| [karukan-im](karukan-im/) | 共有IMEエンジン — ステートマシン、ローマ字変換、karukan-imserver(macOS向けJSON-RPCサーバー) |
| [karukan-engine](karukan-engine/) | コアライブラリ — ローマ字→ひらがな変換 + llama.cppによるニューラルかな漢字変換 |
| [karukan-cli](karukan-cli/) | CLIツール・サーバー — 辞書ビルド、Sudachi辞書生成、辞書ビューア、AJIMEE-Bench、HTTPサーバー |

## 特徴

- **ニューラルかな漢字変換**: GPT-2ベースのモデルをllama.cppで推論し、高度な日本語変換
- **ライブ変換**: 入力と同時に変換結果をリアルタイム表示。Spaceを押さずに変換が進む（`Ctrl+Shift+L` でON/OFF）
- **コンテキスト対応**: 周辺テキストを考慮した日本語変換
- **変換学習**: ユーザーが選択した変換結果を記憶し、次回以降の変換で優先表示。予測変換（前方一致）にも対応し、入力途中でも学習済みの候補を提示
- **システム辞書**: [SudachiDict](https://github.com/WorksApplications/SudachiDict)の辞書データからシステム辞書を構築
- **候補リライター (Mozcから移植)**: 半角カタカナ、英字の大文字小文字・全角半角、記号の関連候補、数字の各種表記（漢数字・大字・ローマ数字・丸数字・16/8/2進数）を自動生成。各候補にはMozc由来の注釈（「半角カタカナ」「16進数」など）が付く
- **絵文字入力**: かな読み（`ぴえん` → 🥺、`きんにく` → 💪）と Slack 風 `:trigger` クエリ（`:smile` → 😄、`:halo` → 😇）の両方をサポート

> **Note:** 初回起動時にHugging Faceからモデルをダウンロードするため、初回の変換開始までに時間がかかります。2回目以降はダウンロード済みのモデルが使用されます。

## インストール

- **Linux (fcitx5)**: [karukan-fcitx5 の README](karukan-fcitx5/README.md#install) を参照
- **macOS**: [karukan-macos の README](karukan-macos/README.md) を参照

## ライセンス

MIT OR Apache-2.0 のデュアルライセンスで提供しています。

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

[karukan-engine/data/](karukan-engine/data/) 配下には [Mozc](https://github.com/google/mozc)（Google製日本語入力システム）から派生したデータを含み、こちらは [BSD 3-Clause License](http://opensource.org/licenses/BSD-3-Clause) のもとで配布されています。各派生ファイルの由来およびMozcの著作権表記は [THIRD_PARTY_LICENSES](THIRD_PARTY_LICENSES) を参照してください。
