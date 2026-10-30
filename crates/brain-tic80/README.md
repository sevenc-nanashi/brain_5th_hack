# TIC-80

第5世代 Brain / PW-H1 向けの TIC-80 Lua カートリッジプレイヤー。
TIC-80 v1.1.2837 のコアと、その版が指定する Lua・blip-buf・zlib を使用する。
Reflex が描画・キー／タッチ・終了処理を担当する。
音声は停止の切り分けのため一時的に無効化している。実機では音声合成・再生・自動再起動を行わない。

## ビルドと配置

```sh
make tic80
```

初回はソースと ARM 用 newlib を取得し、SHA-256 を検証する。
Ruby、curl、tar、dpkg-deb、clang-20、llvm-ar-20、llvm-objcopy-20、
Rust の `armv7a-none-eabi` ターゲットが必要。

`target/tic80/APP/TIC80/` を SD カードの `APP/TIC80/` にコピーする。
生成物は `AppMain.bin`、`index.din`、`cart.tic`、`LICENSES.txt`。
同梱の `cart.tic` は描画・入力・効果音を確認する Reflex Orbit デモ。

起動時は `SD0:\APP\TIC80\cart.tic` を読む。
別のゲームは SD カードの任意のフォルダーに `.tic` ファイルとして置き、
**一覧キーで Explorer を開いて選択**する。フォルダーと `.tic`（大文字拡張子も可）だけを表示する。
方向キーで移動、Enter またはタッチでフォルダーを開く／ゲームを選択する。
一覧表示中はゲームを停止する。一覧キーで閉じると続きから再開する。
Esc は親フォルダーへ戻り、Storage 画面では選択をキャンセルする。
選択したフォルダーは次に一覧を開くときも保持する。
別のゲームを選ぶと現在のプレイ状態・起動中の `pmem()` は破棄される。
起動時や読込時のエラー画面からも一覧キーで選び直せる。
再ビルドした配布フォルダーを丸ごとコピーすると `cart.tic` もデモに戻るため、
ゲームを保持したい場合は `AppMain.bin` と `LICENSES.txt` のみ更新する。

出力先を変える場合は `make tic80 BUILD_DIR=/tmp/brain-tic80-build`。
直接 Cargo を使う場合は、取得先を `TIC80_INPUTS` に指定する。

## 操作

| Brain | TIC-80 |
| --- | --- |
| 方向キー | ゲームパッドの上下左右 |
| Z / X / A / S | A / B / X / Y ボタン |
| 英数字・記号・Space・Enter・Backspace・Shift | キーボード入力 |
| タッチ／ドラッグ | マウス移動・左ボタン |
| 一覧から選ぶ | ゲーム選択を開く／閉じる |
| 履歴 | デバッグメニューを開く／閉じる |
| My辞書 | 上端のFPS表示を切り替える |
| Esc / ホーム | 終了 |

アルファベットが入力されない場合は本体の「あA1/記号」で切り替える。
キーボードAPIへの同時入力は TIC-80 の仕様に合わせ最大4キー。
ゲームパッドの押下状態は別に保持する。

## デバッグ表示

上段の「履歴」で計測メニューを表示する。表示中もゲームは進行し、
ゲームへの操作入力だけを止める。「履歴」または Esc で閉じる。
「My辞書」はメニューを閉じた状態でもFPS表示を切り替えられる。
メニュー内の R で計測値をリセットする。ホームはアプリ終了、一覧はゲーム選択。

- `Display FPS` / `FPS`: 新しいゲーム画面を実際に描画した回数／秒。同じ画面のUI再描画は数えない。
- `Emulation FPS` / `EMU`: ゲーム内部で実行したフレーム数／実時間1秒。等速時は約60。
- `Audio fill` / `max`: 直近／最大の音声補充時間（ゲーム実行・合成・submitの合計、マイクロ秒）。
- `Audio submits`: 補充成功回数。`Audio restarts`: ストール検知による再起動の試行回数。
- `Audio state`: 音声APIの生状態値。現在の無音版では `OFF`。音声カウンターは0のまま。

FPSは約1秒ごとに更新する。ゲーム選択中の停止時間はFPSから除外する。
ホストプレビューでは `fps.ppm` と `debug.ppm` も出力する（FPS計算確認用の30/60、音声値は未計測）。

## 実装と対応範囲

- ボーダー込み256×144を3倍の768×432へ拡大し、暫定800pxの利用可能幅に中央配置。
- Lua の通常 `.tic` 形式、スプライト・マップ・パレット・効果音・音楽、8バンクに対応。
  最大ファイルサイズは1 MiB。チャンク境界・コード長・言語を検証してから上流ローダーへ渡す。
- PNGカートリッジ、旧形式の圧縮コード／カバー／パターン、Lua以外の言語は対象外。
  PCのTIC-80で通常の `.tic` として保存し直す。
- `pmem()` は起動中のみ保持し、SDカードには保存しない。エディター・ネット接続は含めない。
- 実機では固定の60Hz周期でゲームを進める。通知の遅れは1回最大4フレームまで追いつき、最新の画面を描画する。
  それ以上の遅れは破棄して操作への応答を保つ。描画FPSは実機のタイマー頻度・描画負荷に依存する。
  `time()` は実行したフレーム数に基づくシミュレーションの経過時間を返す。
  音声の補充コードは残しているが、実機バイナリには組み込まない。ホストのPCMプレビューは維持する。
- コア・Lua・newlib の malloc/free/realloc/calloc を Reflex の追跡型アロケーターへ接続。
  Cエンジンは専用256 KiBスタックで実行する。メモリ不足などの回復不能なCエラーはpanic画面で停止する。
- Luaエラーは画面に表示する。通常の暴走スクリプトを検出するため1フレーム約2000万命令で中断する。
  この上限はセキュリティサンドボックスではなく、Luaのdebug APIやC関数内の処理までは制限しない。
- 終了時はタイマーを停止し、コアを解放する。
- 上流ソースの変更はビルド時のコピーに限定し、Luaの命令フック・初期化失敗時のVM解放・終了時のRAM解放・
  非整列チャンクヘッダーの安全な読み込みを追加する。

## ホストでの確認

```sh
make tic80-preview
```

デモを120フレーム実行し、`target/tic80-preview/tic80.ppm` と `tic80.wav` を生成する。
不正なカートリッジ、Luaエラー、命令上限、`exit()`、終了後の再初期化も assertion で確認する。
別のカートリッジを確認する場合：

```sh
TIC80_INPUTS="$PWD/target/tic80-inputs" cargo run -p brain-tic80 \
  --example preview --features engine --release -- game.tic target/tic80-preview
```

```sh
cargo fmt --all
TIC80_INPUTS="$PWD/target/tic80-inputs" cargo clippy -p brain-tic80 --features engine --all-targets -- -D warnings
TIC80_INPUTS="$PWD/target/tic80-inputs" cargo clippy -p brain-tic80 --features device --target armv7a-none-eabi -- -D warnings
```

音声有効版では実機で周期的な停止と再起動を確認したため、現在は無音版で切り分けている。
無音版の実機での速度・連続動作は未確認。

## ソースとライセンス

取得元とハッシュは `scripts/fetch-tic80.rb`、ビルドと上流パッチは `scripts/build-tic80.rb`。
ライセンスは配布フォルダーの `LICENSES.txt` に同梱する。
blip-buf のライセンスに従い、再リンク用 `libbrain-tic80.a` は Cargo のビルド出力に保持する。
ゲームのデータは各作品のライセンスに従う。
