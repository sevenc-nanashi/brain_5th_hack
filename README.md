# brain_5th_hack

Brainの第5世代（PW-x1）で遊ぼう！

## Crates

| crate                                                     | 内容                                                     |
| --------------------------------------------------------- | -------------------------------------------------------- |
| [brain-rom-dumper](crates/brain-rom-dumper/README.md)     | SDU 更新パッケージの検証・展開                           |
| [brain-app-analyzer](crates/brain-app-analyzer/README.md) | AppMain.bin の起動コード・初期化テーブルの解析           |
| [reflex](crates/reflex/README.md)                         | 第5世代 Brain アプリ向けの実行基盤・描画・入力・音声 API |
| [reflex-cli](crates/reflex-cli/README.md)                 | アプリのビルド・SD カードへの配置                        |
| [brain-calculator](crates/brain-calculator/README.md)     | 関数電卓                                                 |
| [brain-hello-world](crates/brain-hello-world/README.md)   | Hello world の描画デモ                                   |
| [brain-doom](crates/brain-doom/README.md)                 | Reflex を使う Doom                                       |
| [brain-inspector](crates/brain-inspector/README.md)       | 画面・入力・音声の計測アプリ                             |
| [brain-explorer](crates/brain-explorer/README.md) | フォルダー移動・ファイル内容の閲覧 |

コマンドはリポジトリのルートで実行する。各 crate の使い方・ビルド手順はリンク先を参照。

## 解析資料

- [SDU フォーマット](docs/sdu-format.md)
- [アプリバイナリ形式](docs/app-binary-format.md)
- [音声 API](docs/audio.md)
- [ファイル操作 API](docs/filesystem.md)

## 開発

```sh
cargo fmt --all
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
