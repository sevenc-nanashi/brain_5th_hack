# brain_pwh1_hack

Sharp Brain PW-H1 の更新データを解析する Rust monorepo。
Cargo workspace の各ツールを `crates/` に配置する。

## brain-rom-dumper

SDU の構造・チェックサムを検証し、更新段階ごとにファイルを取り出す。
検証対象は `SH_DIC_UPDATE_V690_H1.SDU`（136,808,651 bytes）。
これは更新パッケージの展開ツールであり、実機から ROM を読み出すツールではない。

```sh
# メタデータとファイル一覧を JSON で表示
cargo run -p brain-rom-dumper -- info '/mnt/z/SH_DIC_UPDATE_V690_H1(1).SDU'

# 新しいディレクトリへ展開
cargo run -p brain-rom-dumper -- extract '/mnt/z/SH_DIC_UPDATE_V690_H1(1).SDU' target/rom-dump
```

出力先は未作成のディレクトリを指定する。入力を検証してから書き出し、既存の出力先は拒否する。
書き込み中の I/O エラーでは部分的な出力が残るため、別の出力先で再実行する。
入力全体をメモリに読み込むため、入力サイズ以上の空きメモリが必要。

```text
target/rom-dump/
  manifest.json             # 段階、パス、オフセット、サイズ、操作種別など
  header.bin                # 元の SDU ヘッダー
  updater.bin               # 前半の生データ（名称は推定）
  boot.bin
  system.bin
  updates.bin               # 更新レコード領域の生データ
  packages/
    001/ROM/...             # 1.80 → 3.30
    002/ROM/...             # 3.30 → 3.40
```

サンプルには 2 パッケージ、84 レコードがあり、81 件のファイル本体を展開できる。
残り 3 件は本体のない操作レコードとして manifest に残す。同名ファイルも更新段階別に保持する。
前半領域の復号・逆アセンブル、内包ファイルの独自圧縮の展開、実機への更新適用・再パックは行わない。
更新に含まれないファイルは復元できないため、展開結果は完全な ROM イメージではない。

解析結果のフィールド定義と未確定事項は [docs/sdu-format.md](docs/sdu-format.md) を参照。
実データと展開物はリポジトリに含めず、`target/` 配下に保存する。

## brain-app-analyzer

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
詳しい構造・圧縮方式・未確定事項は [docs/app-binary-format.md](docs/app-binary-format.md) を参照。

## Reflex

第5世代 Brain アプリ向けの `no_std` crate、`crates/reflex`。
描画・アプリ終了と、キー／タッチ／タイマーを含む PEG の ABI 定義をまとめる。
PEG の関数呼び出しは V690_H1 未照合のため `experimental-peg` feature が必要。
使い方と確認状況は [Reflex の README](crates/reflex/README.md) を参照。

## brain-hello-world

第5世代 Brain の追加アプリとして白背景に `Hello world!` を表示する Rust の試作。
解析ツールとは別 crate。実機表示は未確認。

```sh
rustup target add armv7a-none-eabi
make hello-world
```

LLVM の `llvm-objcopy-20` が必要。別の名前でインストールされている場合は
`make hello-world OBJCOPY=llvm-objcopy` のように指定する。
出力された `target/hello-world/APP/HelloWorld/` を SD カードの `APP/HelloWorld/` へコピーする。
フォルダ内の `AppMain.bin` と空の `index.din` が必要。PC 上でビルドした ELF をコピーしない。

追加アプリから起動すると、フレームバッファへ描画して有限回待機し、純正アプリと同じ SharpLib の入口へ終了を通知してから戻る。
待機時間は CPU の速度に依存し、秒数は未校正。操作イベントは処理しない。
戻った後のランチャーの描画復旧も未確認。固定 MMIO を使用するため、環境の違いによって
画面が乱れたり停止したりする可能性がある。Windows CE 世代向けではない。

起動・描画の根拠とバイナリの確認方法は [docs/hello-world.md](docs/hello-world.md) を参照。

## brain-doom

Reflex を使う Doom の試作。指定の DOOM1.WAD を埋め込んだ追加アプリを生成する。
PC 上のゲーム開始・操作・終了は確認済み、PW-H1 実機動作は未確認。

```sh
make doom-fetch
make doom
```

`target/doom/APP/Doom/` を SD の `APP/Doom/` へコピーする。
操作・制限・検証手順は [docs/doom.md](docs/doom.md) を参照。

## brain-inspector

画面領域・キーの生コード・タッチ座標を確認する独立した計測アプリ。
`make inspector` で生成し、`target/inspector/APP/Inspector/` を SD の `APP/` へコピーする。
操作と各数値の意味は [docs/inspector.md](docs/inspector.md) を参照。

## 開発

```sh
cargo fmt --all
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
