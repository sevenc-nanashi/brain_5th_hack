# Reflex

第5世代 Sharp Brain 向けの `no_std` Rust crate。crate 名は `reflex`。
解析ツールとは独立し、アプリから使う描画・終了・PEG の ABI 定義をまとめる。
外部依存・アロケーター・書き込み可能な static は持たない。

## 利用

この monorepo 内のアプリからは path dependency で参照する。
crates.io への公開はしていない。

```toml
[dependencies]
reflex = { path = "../reflex" }
```

| モジュール | 内容 | 確認状況 |
| --- | --- | --- |
| `app` | 起動コンテキスト、`exit` | V690_H1 の純正アプリと SharpLib で終了経路を確認 |
| `display` | 854×480 RGB565、回転座標、ピクセル書き込み、キャッシュ同期、表示更新 | 既存 Hello World の処理を移植、実機確認が必要 |
| `peg` | キー・タッチ・タイマーのメッセージ、描画用 ABI 型・vtable スロット | Brain Hackers の公開サンプルに基づく、V690_H1 未照合 |
| `peg::raw` | ウィンドウ生成、画面矩形、描画、フォーカス、タイマー、破棄の関数ポインタ | `experimental-peg` が必要、V690_H1 未照合 |

デバイスを操作する API は `arm` + `target_os = "none"` でのみ公開する。
型・定数・座標変換はホストでも使える。ポインターを含む ABI 型の配置は
32-bit ARM 用であり、64-bit ホストのメモリをそのままデバイスへ送ってはいけない。

```rust,ignore
use reflex::{app, display};

// PW-H1 の起動環境・メモリマップ・表示への排他的アクセスを呼び出し側で保証する。
unsafe {
    display::write_pixel(100, 100, 0xffff);
    display::present();
    // 実際のアプリではここで表示期間を確保する。
    app::exit(context); // ローダーから受け取った元の AppContext ポインター
}
```

`app::exit` は戻る可能性がある。所有するウィンドウ・タイマーなどを解放した後に
一度だけ呼び、ローダーへ戻る。`-> !` として扱わない。
実装例と起動コードは [brain-hello-world](../brain-hello-world/src/main.rs) を参照。

## キー・タッチ・タイマー

`peg::Message` は `repr(C)`、データ部分は C と同じ union。
未知のメッセージも受け取れるよう、種別とキーコードは整数のまま保持する。

| `message.kind` | ペイロード |
| --- | --- |
| `PM_KEY` / `PM_KEY_DOWN` | `param` がキーコード、`data.ext_params[0]` が修飾・押下フラグ |
| `PM_KEY_UP` | キー解放の候補値。元資料でも未確定 |
| `PM_LBUTTONDOWN` / `PM_LBUTTONUP` | `data.point` がタッチ座標 |
| `PM_TIMER` | `param` がタイマー ID |

`peg::key` に Enter、Escape、Space、Backspace、矢印、Shift を定義。
`KMOD_SHIFT`、`KFLAG_PRESS`、`KFLAG_HELD` はメッセージ内のフラグ。
`KFLAG_HELD = 0xc00` は複数ビットなので、判定は `(flags & KFLAG_HELD) == KFLAG_HELD`。
union の読み取りはイベント種別を確認してから `unsafe` で行う。

PEG 関数が必要なアプリだけ次を指定する。

```toml
reflex = { path = "../reflex", features = ["experimental-peg"] }
```

`peg::raw` は固定アドレスを C ABI で呼ぶ `unsafe fn` を公開する。
`dec_window_ctor`、`rect_set`、`screen_rect`、`adjust_client`、
`win_begin_draw`、`win_base_draw`、`get_screen`、`set_current`、
`set_timer`、`kill_timer`、`destroy` が対象。タイマーの周期はミリ秒ではなく PEG tick。
各アドレス・ABI を使用中のファームウェアで照合してから呼び出す。

これは PEG の低レベル定義であり、キーをポーリングする API ではない。
入力を受け取るには、PEG ウィンドウを登録して `VFUNC_MESSAGE` のコールバックを設定し、
フォーカスを与えて起動処理から戻る必要がある。ウィンドウ・コールバック・描画バッファを
有効に保ち、未処理メッセージを元のハンドラーへ渡す実装はアプリ側で行う。
安全なウィンドウ所有権管理やイベントループはまだ提供していない。

起動処理・リンカースクリプトもアプリ側で用意する。
Hello World の起動処理は `.data` / `.bss` を初期化しないため、イベントアプリへ
書き込み可能な static を追加する場合は先に初期化処理が必要。

## 根拠と検証

- [event_dumper/peg_const.h](https://github.com/brain-hackers/lab/blob/5f97124e9d62a0e74698ebe9755f8fb91a60b004/x1/event_dumper/peg_const.h): メッセージとキーの ABI。
- [event_dumper/peg_func.h](https://github.com/brain-hackers/lab/blob/5f97124e9d62a0e74698ebe9755f8fb91a60b004/x1/event_dumper/peg_func.h): PEG 関数・Bitmap ABI。元資料は CC0。
- [Hello World の解析メモ](../../docs/hello-world.md): 描画アドレスと純正アプリの終了経路。

元資料の終了アドレス `0x840e7a48` は採用せず、V690_H1 で確認した公開入口
`0x8401013c` を使う。PEG の一部は実装関数の固定アドレスで、版の違いに弱い。

```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p reflex --all-features --target armv7a-none-eabi -- -D warnings
make hello-world
```

ARM ビルド時に起動コンテキスト・メッセージ・Bitmap のサイズとオフセットを
コンパイル時 assertion で検査する。実機のイベント配送や描画動作までは検査できない。

## 命名

腕（ARM）に熱湯とかをかけると脳（Brain）で意識する前に動かすのを逃避反射（Withdrawal Reflex）というらしいので、それのReflexの部分。
