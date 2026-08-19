# SPEC: upstream 選択移植と円キー composition 不整合

比較日: 2026-08-18
比較基準 (this fork `main`): `3e96b34884f82c93de2dc44a33856c52c87dcf5d`
upstream `main`: `5e1b5411a681c26a9b8bc1f5905035781c0d5bb2`
merge-base: `6746900de5ae44f1cf3b28784235e9b2f22326ee`
upstream のみ: 44 commits / fork のみ: 22 commits
方針: merge/cherry-pick しない。採用は必要箇所の最小移植。本家を fork の Google IME 風 UX に合わせない。

## 円キー症状の根本経路

JIS 円キーは `NSEvent.characters` が U+00A5 (`¥`) になる。`KeyCodeMap.translate` は ASCII 印字 (0x20–0x7e) 以外を `nil` にし、コントローラは `handle` を未消費で返す。IMK は未消費キーをクライアントへ通し、marked text を `¥` で置き換える。エンジン側 `input_buf` は `てすと` のままなので、Enter の `Commit("てすと")` が既に入った `¥` の後へ挿入され、文書は `¥てすと` になる。

同時にエンジンの `Keysym::is_printable` も ASCII のみのため、たとえ keysym `0x00a5` (XK_yen) が届いても composing 中は未消費になる。

既存の `clean_model_output` 先頭記号除外と、明示入力 `\` / `¥` の rewriter 候補は別経路であり、壊してはならない。

期待: 円キーは消費され、composition 表示 (`UpdatePreedit`) と確定 (`Commit`) が同じ `てすと¥` になる。

## 採用

| 本家 | 採用内容 | 理由 | fork 衝突 |
|---|---|---|---|
| #91 `a7705d4` バイトフォールバックを special 扱いしない | `llamacpp.rs` の decode フィルタと往復テスト | 語彙外文字 (Ψ 等) が無言で消える実害。UX 非依存。ファイル局所。 | なし |
| #97 `ab6ef8e` モデルパスをプロセス内で1回解決 | 現行 `hf-hub 0.4` / `ApiBuilder` に合わせて `download_gguf` へメモ化 | 並列テストで snapshot symlink が消える flaky と、毎ロードの HEAD を止める。#101 の大規模バックグラウンド化はしない。 | なし (API 差は適応移植) |
| #103 `320b52a` thin LTO | `Cargo.toml` の `[profile.release]` のみ `lto = "thin"` | 再リンク短縮。推論ホットパスは llama.cpp 側。 | fcitx5 `target-cpu=native` 既定化は配布バイナリ向け新既定なので不採用 |

## 不採用

| 本家 | 不採用理由 |
|---|---|
| #112 変換中矢印で編集復帰 | fork は変換中 ←→ をセグメント移動に使う。非互換。 |
| #101 オフラインでも固まらない | 価値は高いが `init_kanji_converter` 同期ロードと変換キャッシュに依存する fork 経路の書き換えになり、最小移植を超える。リトライ回数変更も含む。 |
| #100 記号・半角全角・スペース設定 | 製品設定の新面。既存 fork UX / 設定の source of truth と衝突しうる。 |
| #98 / #94 ソース絞り込み Ctrl+R/T | fork に無い本家操作。数字 1-9 即確定など Google IME 風 UX と衝突。 |
| #96 候補先頭をライブ変換固定・選択を Ctrl+数字 | fork の予測未選択 / 数字確定と非互換。 |
| #95 変換中タイピングで読みを伸ばす | fork は変換中の印字で確定して次入力。 |
| #87 Chunk 区切り見直しと Ctrl+J 固定 | fork 独自 chunking / ライブ変換デフォルト OFF と衝突。 |
| #88 jinen-v2 デフォルト変更 | 既定モデルは fork `models.toml` の `jinen-v1-small-q5`。既定モデル変更は製品判断。 |
| #85 ライブ変換結果キャッシュ | fork は別実装の変換結果キャッシュ済み。 |
| #82 辞書 predictive | fork はユーザー辞書前方一致を既に持つ。 |
| #80 / #83 打鍵要素列モデル | 入力モデル刷新は fork romaji FSM を置き換える。非最小。 |
| #77 jinen-v1.1-beta opt-in | 既定を変えない登録追加も、利用判断と models.toml 方針が未決。 |
| #64 学習 surface 上限と Ctrl+Del 削除 | 新ショートカットと数値上限。magic number / 製品判断。 |
| #56 JIS 変換キーと右⌘タップ | 右⌘タップ 0.5 秒は新しい magic number。停止条件に抵触。 |
| #46 commit 時 UpdatePreedit 抑制 | fork の macOS 前端は空 preedit で marked text を消す契約。 |
| #97 以外の hf-hub API 刷新 (#101 内) | 現行は `hf-hub 0.4` の `ApiBuilder`。 |
| deps / renovate / FUNDING.yml / コア集約リファクタ | 機能採用ではない、または fork 配置が既に違う。 |

## 検証 (current-tree)

作業ディレクトリ: `.worktree/fix/upstream-yen-display` (main 直編集はガードされるため worktree)。
インストール済み IME の再起動・置換はしていない。installed/live 未検証。

RED: `cargo test -p karukan-im --lib yen_during_hiragana_composition` → FAIL `yen must be consumed so IMK does not replace marked text`

GREEN / 回帰:

| コマンド | 結果 |
|---|---|
| `cargo test -p karukan-im --lib yen_during_hiragana_composition` | 1 passed |
| `cargo test -p karukan-im --lib test_keysym_printable` | 1 passed |
| `cargo test -p karukan-im --lib test_keysym_to_char` | 1 passed |
| `cargo test -p karukan-im --lib explicit_yen_keeps` | 1 passed |
| `cargo test -p karukan-im --lib explicit_backslash_keeps` | 1 passed |
| `cargo test -p karukan-engine --lib byte_fallback` | 2 passed |
| `cargo test -p karukan-engine --lib clean_model_output` | 1 passed |
| `node scripts/dspec/check-ime-yen-input-spec.mjs` | enumeratedCases=153600, 違反 0 |
| `herdr run --label im-lib-tests -- cargo test -p karukan-im --lib` | 276 passed; 0 failed |
| `herdr run --label engine-lib-tests -- cargo test -p karukan-engine --lib` | 176 passed; 0 failed |
| `herdr run --label fcitx5-lib-tests -- cargo test -p karukan-fcitx5 --lib` | 22 passed; 0 failed |
| `herdr run --label engine-decode-roundtrip -- cargo test -p karukan-engine --test kanji_conversion_tests test_decode_keeps_byte_fallback_chars` | 1 passed |
| `herdr run --label macos-keycode-tests-scratch -- swift test --filter KeyCodeMapTests --scratch-path /tmp/karukan-macos-keycode-scratch` | KeyCodeMapTests 13 tests, 0 failures |
| `cargo fmt --all -- --check` | 成功 |
| `git diff --check` | 成功 |

最初の `swift test --filter KeyCodeMapTests` は worktree `.build` が main の module cache を指してコンパイル失敗。ソース失敗ではない。`--scratch-path` で再実行し 13/13 成功。


1. `KeyCodeMap` は ASCII 印字に加え XK_yen (`U+00A5`) だけを keysym 化する。かな (`あ`) は翻訳しない。
2. エンジン `Keysym::is_printable` は XK_yen を印字として消費する。
3. composing 中の円キーは `UpdatePreedit` に既存読み + `¥` を出し、Enter の `Commit` はその表示と一致する。
4. `clean_model_output` の先頭 `\`, `¥`, `￥` 除外は維持する。
5. 明示 `\` 入力の rewriter 候補 `\`, `¥`, `￥` は維持する。
