# Explorer

第5世代 Brain 向けの読み取り専用ファイルエクスプローラー。
SD カード (`SD0:\`) と ROM (`ROM:\`) のフォルダーを移動し、
ファイルをテキスト／16進表示で閲覧する。書き込み・削除・実行は行わない。

## ビルド

リポジトリのルートで実行する。Rust の `armv7a-none-eabi` target と `llvm-objcopy-20` が必要。

```sh
rustup target add armv7a-none-eabi
cargo run -p reflex-cli -- build explorer
# または
make explorer
```

`target/explorer/APP/Explorer/` を SD カードの `APP/Explorer/` にコピーする。
`AppMain.bin` と空の `index.din`、同梱ライセンスを保持する。
`reflex deploy explorer <SDカードのパス>` でも配置できる。

## 操作

| 操作 | キー／タッチ |
| --- | --- |
| 一覧の選択 | 上下キー |
| フォルダー・ファイルを開く | 決定／右キー、行のタップ、Open |
| 前／次ページ | PageUp／PageDown、Prev／Next。内容表示中は上下キーも使用可 |
| テキスト／16進切替 | T、Text／Hex |
| 親フォルダー・一覧へ戻る | Esc／Backspace／左キー、Back |
| 一覧・現在ページの再読込 | R |
| 終了 | Storage 画面で Esc／Backspace／左キー、Exit |

タッチは同じ行・ボタンで離すと確定し、外へ移動すると取り消す。
キーの重複した押下通知は解放まで無視する。
親フォルダーへ戻ると、開いていたフォルダーを選択する。
読み取りに失敗した場合は画面下部にエラーを表示し、元の一覧・ページを保持する。

## 表示と制限

操作領域は800×480。共通の854×480描画バッファの左側だけを使用する。
フォルダーを先に、各種別内ではファイル名の生バイト順に並べる。
一覧は9行、テキストは64列×12行、16進表示は16バイト×16行。
ファイルはページごとに開いて必要な部分だけ読み、閉じる。
テキスト／16進切替は現在のバイト位置を保つ。Prevは実際に通ったページ位置へ戻る。

- 標準フォントはASCIIのみ。非ASCII・制御バイトは `\xNN` として表示し、元の名前のまま開く。
  日本語の字形・文字コード変換は未対応。テキスト内のLF/CRLFは改行、タブは4列区切りの空白として扱う。
- パスは259バイトまで、一覧は4096件まで。上限を超えたフォルダーはエラーにする。
- ファイルのシーク先は `i32::MAX` まで。2GiB以降の閲覧は未対応。
- ファイル名の表示が長い場合は省略する。一覧の選択名は下部にも表示する。
- ファイルシステムAPIはV690_H1の静的解析に基づく。実機動作は未確認。
  列挙APIはEOFとエラーを区別できず、カード取り外し時などに不完全な一覧となる可能性がある。

ABI・アドレス・戻り値の根拠は[ファイル操作 API](../../docs/filesystem.md)を参照。

## ホストプレビュー

Unixホストの実ディレクトリーをSD0として使用し、Storage・一覧・最初のファイルの
テキスト／16進画面をPPMへ出力する。入力は変更しない。プレビューのROMには接続しない。
出力ディレクトリー内の同名PPMは上書きする。

```sh
cargo run -p brain-explorer --example preview -- /path/to/input target/explorer-preview
cargo clippy -p brain-explorer --all-targets -- -D warnings
cargo clippy -p brain-explorer --features device --target armv7a-none-eabi -- -D warnings
```

実機ではSD0/ROMの一覧、ファイル種別とサイズ、フォルダー移動、ページ送り、
空ファイル、カード未挿入、キー・タッチ、終了後の復帰を確認する。
EOFと列挙エラーの区別はAPI側の制限として残る。
