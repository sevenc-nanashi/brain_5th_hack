# 第5世代向け Hello world

`crates/brain-hello-world` は `no_std` の ARM アプリ。
既存の追加アプリの起動規約とフレームバッファ描画の実験を使った、実機確認前の小さなデモ。
ホスト上の解析ツールとは別の crate にし、デバイス用バイナリは `device` feature を指定してビルドする。
通常の workspace check / clippy では共通の描画コードと PC 用プレビューを検査できる。

## 根拠

- [Brain Hackers の samplefb/main.c](https://github.com/brain-hackers/lab/blob/5f97124e9d62a0e74698ebe9755f8fb91a60b004/x1/samplefb/main.c)
  は `0x60A00008` に 16-bit ピクセルを書き、`x * 480 + (479 - y)` で 640×480 の画像を回転している。
  描画後に `0x40AA0004` へ 1 を書く。
- PW-H1 の画面は [公式仕様](https://jp.sharp/support/dictionary/product/pw-h1.html) では 854×480。
  参考サンプルの 640 ピクセル幅は使用せず、854 ピクセル全体を描画・キャッシュ同期する。
- [samplefb/linker.ld](https://github.com/brain-hackers/lab/blob/5f97124e9d62a0e74698ebe9755f8fb91a60b004/x1/samplefb/linker.ld)
  は raw アプリを `0x90000000` にリンクする。
- 手元の AppMain 解析でも追加アプリ群の複数で `0x90000000` のリンクアドレスが得られた。
- 上記リポジトリは [CC0](https://github.com/brain-hackers/lab/blob/5f97124e9d62a0e74698ebe9755f8fb91a60b004/LICENSE)。
  ハードウェア定数と配置の参考に使用した。文字のビットマップと Rust の描画処理はこのリポジトリで作成した。

これらは第五世代での既存の実験に基づく値であり、このデモの PW-H1 実機動作を保証するものではない。

## 実装

描画・キャッシュ同期・終了 API と起動コンテキストの型は
[Reflex](../crates/reflex/README.md) にまとめ、デモ側は描画内容・起動処理・待機を担当する。

1. ARM 状態の `_start` から `hello_main` を呼ぶ。スタックと r0 の起動コンテキストはローダーから受け取る。
2. 854×480 の白背景と、中央の黒い 5×7 ビットマップ文字を描画する。文字は 6 倍に拡大。
3. 480×854 の物理バッファへ回転した座標で volatile 書き込みを行う。
4. CTR の DminLine からキャッシュライン長を得て、書き込んだ範囲を DCCMVAC で clean し、DSB を実行する。
5. 表示更新用レジスタへ 1 を書いて DSB を実行する。
6. `0x10000000` 回の有限待機後、`sh_ExitApp(context->exec_path)` を公開入口 `0x8401013C` 経由で呼び、保存した LR へ戻る。

MMU・割り込み設定は変更しない。CPU の特権命令が許可される第五世代の起動環境を前提とする。
キャッシュ同期は既存サンプルの OS 関数ポインタを使わず、ARM 命令で実施する。
待機中はキーボードやタッチを処理しないため、常用アプリのイベントループには適さない。

書き込み可能な static データは持たず、RAM の `.data` 展開や `.bss` 初期化を不要にした。
これらの領域が追加されたらリンク時の ASSERT でビルドを止める。
文字データは raw イメージの `.text` にコードと合わせて格納する。

## 純正アプリに合わせた終了処理

既存サンプルの `sh_ExitApp = 0x840E7A48` は採用していない。
V690_H1 の `SharpLib.bin` のファイル位置 `0xE7A48` は比較・検索処理の途中であり、
同じベースアドレスでロードされる前提では当該関数の入口と一致しない。
実装関数を直接呼ぶ代わりに、純正アプリが使用する公開入口 `0x8401013C` を使う。

V690_H1 で確認した経路（ファイル内オフセット）は次のとおり。

| バイナリ | 位置 | 処理 |
| --- | --- | --- |
| PCConnect/AppMain.bin | `0x14670` | 保存済みコンテキストを読み、`+4` の実行パスを r0 に返す |
| PCConnect/AppMain.bin | `0x14978` / `0x1497C` | 上記関数の返り値を引数に終了処理を呼ぶ |
| PCConnect/AppMain.bin | `0x14758` / `0x1475C` | `LDR pc, [pc, #-4]` とアドレス `0x8401013C` |
| SharpLib.bin | `0x1013C` | `B 0xE6BDC`（命令 `0xEA035AA6`） |
| SharpLib.bin | `0xE6BDC` | 実行パスでアプリ登録を探し、関連する領域を解放 |
| SharpLib.bin | `0xE6C64`〜`0xE6C74` | 種別 `0x10E`、パラメーター `2` の終了通知を送る |
| SharpLib.bin | `0xE6C7C` | `sh_ExitApp : Type=PM_DICEVT_APP, Param=EVENT_APP_FINISH` のログを参照 |

この終了関数には戻り経路があるため、Rust 側は `extern "C" fn(*const u8)` として呼ぶ。
呼び出し後は通常どおりローダーへ戻す。単なる return では終了通知が欠けていた。
起動コンテキストは保持するが、書き込み可能な static 領域は追加しない。

変更後の画面復旧は実機確認が必要。PEG のウィンドウは作成していないため、その破棄処理も追加していない。

## ビルドと生成物

リポジトリのルートから実行する。

```sh
rustup target add armv7a-none-eabi
make hello-world
```

`CARGO`、`OBJCOPY`、`BUILD_DIR` は make の変数で指定できる。
デバイス向けコンパイルは ARMv7-A / Cortex-A7、soft-float、panic=abort、static relocation。
`link.x` の ASSERT でエントリーポイントを `0x90000000` に固定する。
`llvm-objcopy -O binary` で ELF のヘッダーを除いた raw バイナリを作る。

```text
target/hello-world/APP/HelloWorld/
  AppMain.bin
  index.din       # 空ファイル
```

`HelloWorld` フォルダを SD カードの `APP` 配下へコピーし、追加アプリから起動する。
ファームウェアの書き換えや `.SDU` の再パックは不要。

バイナリは ARM の `PUSH {r4, lr}` から始まる。
未解決シンボル・再配置エントリー・書き込み可能 static 領域がないことを確認した。
ホスト版・ARM 版の clippy と、同じ描画コードを使う PC プレビューを確認した。
プレビューは実機画面の写真ではなく、色・字形を確認するためのもの。

このアプリは純正アプリの初期化テーブルを持たないため、現在の `brain-app-analyzer` の対応対象外。
デモの検証には ELF のセクション・エントリー・逆アセンブル結果を使う。
