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

| キー                      | 操作                                      |
| ------------------------- | ----------------------------------------- |
| ↑ / ↓                     | 前進 / 後退、メニュー選択                 |
| ← / →                     | 左右旋回、設定変更                        |
| Z                         | 射撃                                      |
| 空白                      | ドアを開ける・使用。メニューでは決定      |
| A / D                     | 左右へ平行移動                            |
| Shift                     | 走る（移動キーと併用）                    |
| Q / W / E / R / T / Y / U | 武器1〜7（ゲーム中のみ）                  |
| Del                       | メニュー開閉 / 戻る                       |
| あA1/記号                 | マップ（Tab相当）                         |
| 音声                      | 一時停止 / 再開                           |
| PageUp / PageDown         | 本体音量を0〜9で変更（Shift併用でも同じ） |

上段6キーは左からF1〜F6、Shift併用でF7〜F12になる。

| 上段キー  | 通常                    | Shift併用                    |
| --------- | ----------------------- | ---------------------------- |
| ホーム    | F1: Doomのヘルプ        | F7: ゲーム終了確認           |
| 国語/漢字 | F2: 無効                | F8: メッセージ表示切替       |
| 英和/和英 | F3: 無効                | F9: 無効                     |
| My辞書    | F4: 効果音・BGM音量設定 | F10: アプリ終了確認（Y / N） |
| 履歴      | F5: 照準切替            | F11: 明るさ切替              |
| 一覧      | F6: 無効                | F12: リマップヘルプ          |

F2/F3/F6/F9は保存・読み込み未対応のため無効。
F12のヘルプ表示中はゲーム更新とゲーム時間を止め、F12またはDelで閉じる。Rで音声デバイスを停止・クローズして開き直す。キーリピートでは再実行しない。成功時は `R: Reset audio (OK)` を表示し、ゲームの進行とMUSシーケンサーは維持する。停止を確認できない場合は再初期化せずエラー画面に移り、Qで終了を試みる。
ゲーム画面の左側80pxの余白に、実測FPS（約1秒ごとに更新）と `MIX MAX`（音声初期化後のバッファ補充時間の最大値、ms）を6×13pxのドット文字で表示する。ヘルプを開いている間はヘルプ内の診断表示を使う。
FPSはゲーム画面の更新回数から計算し、ヘルプで停止した時間は除外する。
再生中は250msごとに入力カーソルを監視する。`[0, 4096]` または `[4096, 0]` のまま再生バイト位置も1秒間進まなければ、停止確認・クローズ・再初期化を行う。F12表示中も監視する。音切れの原因修正ではなく、停止後の復旧処理。

F12内の音声診断は約1秒ごとに更新する。リセット時に `Mix ms` と `Submit` をクリアし、新しいストリームの初期補充から計測する。OKは再初期化APIの成功を示し、実際に鳴っていることの保証ではない。4096バイトは約93ms分で、補充時間がこれに近いほどゲーム処理の余裕が少ない。
DejaVu Sans Monoの20px・8bitグレースケール文字をRGB565へ合成し、大小文字を区別する。
フォントのライセンスは `crates/brain-doom/assets/FONT-LICENSE.txt`。
アトラスの再生成は `ruby scripts/doom-help-font.rb /path/to/DejaVuSansMono.ttf`（ImageMagickが必要）。

アルファベット入力モードで操作する。メニュー・確認画面ではQWERTYUを文字として扱う。
押下時の割り当てを保持し、Shiftを先に離した場合やメニューを開いた場合も同じキーを解放する。
Qの即終了はエラー画面だけに限定し、通常の終了はShift+My辞書（F10）→Yで行う。

PW-H1 の実機報告に合わせ、上を `0x25e`、左を `0x260` に修正した。
下は `0x25f`、右は `0x261`。参考サンプルの上・左のラベルとは異なる。
キー解放は `PM_KEY_UP = 34` を使用し、実機では離すと停止することが報告されている。

## 実装

起動・ウィンドウ登録・描画・終了は `#[reflex::main]` / `Application` と共通 `reflex/link.x` を使う。
ゲーム状態は `Doom` が所有し、PEGタイマーと音声通知を `raw_event` で受け取る。
Cエンジン用128KiBスタックの切り替えだけは `engine-stack.S` に残す。
終了時は `can_exit` で音声停止を確認し、失敗時はアプリを保持してエラー画面へ戻る。

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
  消費通知ごとに512フレーム×2回を混合し、4096バイトずつ補充する。BGMも同じPCMストリームへ合成する。
  32768バイトでは実機の補充時間が最大689ms、音声再生中1〜2 FPS（停止後20 FPS）と報告された。OPL音源をOpalへ変更し、描画を長時間止めないよう4096バイト（約93ms）へ短縮する。実機での改善・連続再生は再確認が必要。
  音量はホストの設定を引き継ぐ。PageUp/PageDownはInspectorで実機確認済みのReflex音量APIで変更し、終了後も設定を保持する。
  メニューの SFX Volume は Doom 内部の音量で、ホストの0〜9とは別。
- 終了時は音声停止・停止状態の確認・ハンドル解放、タイマー停止、エンジンのメモリ解放を行う。その後Reflexがフォーカス復元、元のROM vtable復元、ウィンドウ破棄、Rust所有値の解放、SharpLibの終了通知を行う。
- WAD は読み取り専用のメモリ内ファイルとして公開。設定・セーブの書き込み要求は失敗を返す。
  ネット対戦・タッチ操作・保存機能は未対応。
- エンジンまたは音声エラー時は `DOOM ERROR  Q EXIT` を表示。Q または Esc で終了を試みる。
  音声停止を確認できない場合は画面に残り、再生中のメモリを解放しない。
  ハードウェアアドレスの不一致など、エンジン外の停止を捕捉するものではない。

音声 ABI の根拠、実機確認が必要な項目、約13.5時間の連続再生上限は [音声解析メモ](../../docs/audio.md) を参照。

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

| 用途                         | 公開アドレス / 配置         | 根拠（ファイル位置）                         |
| ---------------------------- | --------------------------- | -------------------------------------------- |
| malloc / free                | `0x600063c4` / `0x600063c8` | `0x14660` / `0x14668` の import thunk        |
| ウィンドウ生成               | `0x6000486c`                | `0x14718` の thunk、`0x14810` の呼び出し     |
| タイマー管理オブジェクト取得 | `0x6000413c`                | `0x15654` の thunk                           |
| タイマー停止                 | `0x60005d08`                | `0x1565c` の thunk、`0x15664` の wrapper     |
| ウィンドウ破棄               | `0x60005c9c`                | `0x14780` の thunk、`0x14d90` からの呼び出し |
| 現在のウィンドウ設定         | presentation `+0x18c`       | `0x1564c` の `STR r1,[r0,#0x18c]`            |

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
[PureDOOM-LICENSE.txt](engine/PureDOOM-LICENSE.txt) に保存する。
[上流 README の DOOM LICENSE](https://github.com/Daivuk/PureDOOM/blob/355cfbd16fac119718879239336ee2ea408886bd/README.md#doom-license)
は元の DOOM Source Code License。GPL 版と同一のライセンスだとは扱わない。
WAD と C エンジンをこのリポジトリのソースとして再収録せず、取得元・ハッシュを固定する。

## BGM

WAD内のMUSスコアとGENMIDI音色を使い、FM音源で再生する。別のSoundFontは不要。
F4（My辞書）で効果音・BGMを個別に調整でき、PageUp/Downは本体音量を変更する。
曲の切り替え・ループ・ゲームのPause/Resumeに対応する。

シーケンサーと音色制御はRust、OPL演算は [Opal（libADLMIDIのC版）](https://github.com/Wohlstand/libADLMIDI/tree/3e9ddb13ad39e2de0ca7eb003e622d4741aa50b7/src/chips/opal)。パブリックドメインの宣言は同梱の `OPAL-LICENSE.txt` を参照。
`make doom-fetch` でハッシュ固定のソース・ライセンスを取得する。
`make doom` はライセンスを配布用APPフォルダーにもコピーする。
再リンク用の `libdoom.a` はCargoのbuild出力に残り、音源は取得したソースから再ビルドできる。

MUSを140Hzで進め、18声のOPL3へ割り当てる。2声音色、打楽器、ベンド、
音量・パン・エクスプレッション・サステインを扱う。パンは左・中央・右の3段階。
リバーブ・コーラス・モジュレーション・ソフトペダルは未実装。
OPLは整数演算、周波数・音量テーブルの浮動小数点計算はビルド時だけ行う。
声が足りない場合は古い声を打ち切るため、元のDMXドライバーと完全同一の音にはならない。Opalはチップの打楽器モードに未対応だが、ここではGENMIDIの打楽器音色を通常のFM声で鳴らすため使わない。Nukedからの変更で音色に差が出る。

PC側で発砲なしの30秒間のBGM合成と終了処理を確認した。
WAD内の全13曲について、曲末からのループ・一時停止・音量ゼロのPCM出力も確認済み。
Opalの実機での演算負荷・音切れ・効果音との音量バランスは未確認。PC上の18音同時発音30秒分の比較では、Nuked 0.716秒に対しOpal 0.454秒（約37%短縮）。この比率は実機での速度改善を保証しない。
