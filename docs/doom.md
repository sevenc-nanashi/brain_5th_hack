# Doom on Reflex

`crates/brain-doom` は第5世代 Brain PW-H1 向け Doom の試作。
[PureDOOM](https://github.com/Daivuk/PureDOOM/tree/355cfbd16fac119718879239336ee2ea408886bd)
の C エンジンを、Rust の入力・描画・メモリ管理と Reflex に接続する。
PC ではゲーム開始・移動・射撃・終了を確認済み。PW-H1 実機でも動作とキー解放の報告あり。
方向・速度と、ホスト描画時の画面復旧を修正した版は、実機での再確認が必要。

## ビルド

必要なものは Rust の `armv7a-none-eabi` target、Ruby、curl、Clang/LLVM 20。
リポジトリのルートで実行する。

```sh
rustup target add armv7a-none-eabi
make doom-fetch
make doom
```

`doom-fetch` は PureDOOM の固定コミットと、指定された
[DOOM1.WAD](https://github.com/nneonneo/universal-doom/blob/main/DOOM1.WAD)
を `target/doom-inputs/` に取得し、両方の SHA-256 を検証する。
既存ファイルのハッシュが違う場合は上書きせず停止する。
`doom` 自体はネットワークへアクセスしない。

出力:

```text
target/doom/APP/Doom/
  AppMain.bin
  index.din
```

`Doom` フォルダを SD カードの `APP/Doom` にコピーして、追加アプリから起動する。
WAD は `AppMain.bin` に埋め込むため、別途 SD カードへ配置する必要はない。
この版は Doom 1 の IWAD 専用で、Doom II / PWAD / MOD は扱わない。

`CARGO`、`OBJCOPY`、`BUILD_DIR` は make 変数で指定できる。
C ツールの名前が違う場合は環境変数 `DOOM_CC` / `DOOM_AR` を指定する。
Cargo を直接呼ぶ場合は `DOOM_INPUTS` に素材ディレクトリの絶対パスを指定できる。
ホスト向けの通常の workspace check は素材や C コンパイラーを必要としない。

## 操作

| キー | 操作 |
| --- | --- |
| Esc | Doom のメニュー |
| Enter | 決定 |
| ↑ / ↓ | 前進 / 後退、メニュー選択 |
| ← / → | 左右旋回 |
| Z | 射撃 |
| Space | ドアを開ける・使用 |
| A / D | 左右へ平行移動 |
| X | 走る（移動キーと併用） |
| 1〜7 | 武器選択 |
| Q | Brain のランチャーへ戻る |

PW-H1 の実機報告に合わせ、上を `0x25e`、左を `0x260` に修正した。
下は `0x25f`、右は `0x261`。参考サンプルの上・左のラベルとは異なる。
キー解放は `PM_KEY_UP = 34` を使用し、実機では離すと停止することが報告されている。

## 実装

- エンジンは 320×200。RGB565 へ変換し、元のピクセル縦横比を補正して
  640×480 に拡大、ホスト UI を除いた利用可能幅800pxの中央へ描画する。
  描画範囲は x=80〜719、中心は x=400。利用可能領域内の左右の黒帯は各80px。
- RGB565 のゲーム画面をアプリ内に保持し、PEG の BeginDraw → Bitmap → EndDraw 経由で表示する。
  ウィンドウの Draw callback でも同じ画面を再描画する。液晶への直接書き込みと
  PEG の内部画面が食い違い、Shift 表示などのホスト描画で白くなる問題への対処。
- PEG ウィンドウを登録して起動処理から戻り、キーとタイマーのコールバックで動く。
  タイマーは 1 PEG tick ごと。TSTMR の経過時間を使い、エンジンの時刻は 35 Hz へ変換される。
  描画周期は 35 fps 以下に制限する。実際の fps と速度は実機測定が必要。
- `.data` は raw イメージから純正 PCConnect と同じ `0x90500000` へコピーし、
  起動時に `.bss` をゼロにする。
  エンジンは専用 128 KiB スタック上で実行。C と Rust は ARM soft-float ABI を使用する。
- C エンジンは `-fsigned-char` でコンパイルする。Doom の `ticcmd_t` は移動量を
  plain `char` に格納するため、ARM の既定の unsigned char では後退 `-25` が前進 `231` になる。
  コンパイル時 assertion で signed char を必須にしている。
- C の終了処理は C 内の setjmp/longjmp 境界に戻る。Rust フレームを飛び越えて終了しない。
  メモリ確保失敗も同じ経路を使う。
- エンジンのメモリ確保はシステム malloc/free を使い、割り当てを追跡して終了時に全解放。
  Doom の zone heap は ARM で 6 MiB、それ以外に画面・テーブル等を使用する。
- 効果音は11025Hz・16bit・ステレオ。Reflex の実験的な音声 API で連続再生し、
  消費通知ごとに512フレーム×2回を混合し、4096バイトずつ補充する。BGM は未対応。
  Inspectorで安定再生を確認した4096バイトに内部バッファも合わせる。1本約93msで、複数バッファ分の遅延は残る。Doomでの発音は実機確認待ち。
  音量はホストの設定を引き継ぐ。Shift+PageUp/PageDown はホスト側の処理へ渡す。
  メニューの SFX Volume は Doom 内部の音量で、ホストの0〜9とは別。
- 終了時は音声停止・停止状態の確認・ハンドル解放、タイマー停止、フォーカス復元、元の ROM vtable 復元、ウィンドウ破棄、
  エンジンのメモリ解放、SharpLib の終了通知の順に実行する。
- WAD は読み取り専用のメモリ内ファイルとして公開。設定・セーブの書き込み要求は失敗を返す。
  音楽・ネット対戦・タッチ操作・保存機能は未対応。
- エンジンまたは音声エラー時は `DOOM ERROR  Q EXIT` を表示。Q または Esc で終了を試みる。
  音声停止を確認できない場合は画面に残り、再生中のメモリを解放しない。
  ハードウェアアドレスの不一致など、エンジン外の停止を捕捉するものではない。

音声 ABI の根拠、実機確認が必要な項目、約13.5時間の連続再生上限は [audio.md](audio.md) を参照。

## 確認方法

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p brain-doom --features device --target armv7a-none-eabi -- -D warnings
make doom-preview
```

`doom-preview` は PC 上で同じ C エンジン・WAD・Rust の変換処理を実行する。
キー入力でゲーム開始・移動・射撃を行い、`target/doom-preview.ppm` を新規作成する。
さらにメニューから終了し、終了コールバックで正常に戻ったことを検査する。
既存のプレビューは上書きしないので、再実行時は前のファイルを移動する。
これは実機画面の写真ではなく、エンジンと描画処理の検証用。

ARM ELF ではエントリー `0x90000000`、再配置なし、未解決シンボルなしを確認する。
`AppMain.bin` は約 4.4 MiB、静的領域は `.bss` と RGB565 画面を含め約 5.8 MiB。
横移動の入力処理は C エンジンの `G_BuildTiccmd` で A=-24、A+X=-40、D=24、
解放時=0 を確認。ARM バイナリでも前後・左右の移動量を符号付きで読み出すことを確認した。
ヒープ使用量・ホスト描画後の復旧・修正後の方向と移動速度は実機での確認が残る。

## Reflex の公開入口の照合

V690_H1 の `PCConnect/AppMain.bin` で確認したもの:

| 用途 | 公開アドレス / 配置 | 根拠（ファイル位置） |
| --- | --- | --- |
| malloc / free | `0x600063c4` / `0x600063c8` | `0x14660` / `0x14668` の import thunk |
| ウィンドウ生成 | `0x6000486c` | `0x14718` の thunk、`0x14810` の呼び出し |
| タイマー管理オブジェクト取得 | `0x6000413c` | `0x15654` の thunk |
| タイマー停止 | `0x60005d08` | `0x1565c` の thunk、`0x15664` の wrapper |
| ウィンドウ破棄 | `0x60005c9c` | `0x14780` の thunk、`0x14d90` からの呼び出し |
| 現在のウィンドウ設定 | presentation `+0x18c` | `0x1564c` の `STR r1,[r0,#0x18c]` |

`system.bin` のコード領域では `runtime = file_offset + 0x60002400` として
上記 export とその分岐先・vtable のポインターが一致する。
タイマー登録 `0x60005d04` の分岐先（ファイル `0x32d04`）は
`(timer_manager, window, id, count, reset)` を受け取り、count/reset を 20 倍する。
タイマー配送の `0x32ed0` はメッセージ種別 32 を生成する。
このため、参考サンプルの直接実装アドレス `0x600470a0` / `0x600470d0` は使わない。

TSTMR の MMIO は [event_dumper/main.c](https://github.com/brain-hackers/lab/blob/5f97124e9d62a0e74698ebe9755f8fb91a60b004/x1/event_dumper/main.c)
に基づく。low/high の読み取り間だけ割り込みをマスクし、元の状態に戻す。
この読み順と割り込みの要件は [NXP SDK の変更履歴](https://mcuxpresso.nxp.com/mcuxsdk/25.12.00-pvw2/html/_assets/boards/kw45b41zloc/mcuxsdk-kw45b41zloc.pdf)
も参照。PW-H1 のクロック設定の実測はまだ行っていない。

## 上流ソースとデータ

PureDOOM のコミットは `355cfbd16fac119718879239336ee2ea408886bd`。
ソースは `target/doom-inputs/PureDOOM.h` に保持し、元のライセンス本文は
[PureDOOM-LICENSE.txt](../crates/brain-doom/engine/PureDOOM-LICENSE.txt) に保存する。
[上流 README の DOOM LICENSE](https://github.com/Daivuk/PureDOOM/blob/355cfbd16fac119718879239336ee2ea408886bd/README.md#doom-license)
は元の DOOM Source Code License。GPL 版と同一のライセンスだとは扱わない。
WAD と C エンジンをこのリポジトリのソースとして再収録せず、取得元・ハッシュを固定する。
