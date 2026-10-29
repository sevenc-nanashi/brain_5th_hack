# brain-app-analyzer

コマンドはリポジトリのルートで実行する。

展開済み `AppMain.bin` の起動コードと初期化テーブルを解析する独立した crate。
推定ロードアドレス、RAM の配置、圧縮データの位置・サイズを JSON で表示する。

```sh
cargo run -p brain-app-analyzer -- \
  target/rom-dump/packages/001/ROM/App/RomanTyping/AppMain.bin \
  --data-output target/roman-data.bin
```

`--data-output` を省略すると解析結果だけを表示する。指定時は RAM 初期値を新規ファイルへ展開する。
V690_H1 に含まれる 14 個の AppMain で確認済み。未知の起動処理や伸長ルーチンはエラーにする。
`DicMain.bin`、`SharpLib.bin` の解析は対応対象に含めない。
詳しい構造・圧縮方式・未確定事項は [docs/app-binary-format.md](../../docs/app-binary-format.md) を参照。
