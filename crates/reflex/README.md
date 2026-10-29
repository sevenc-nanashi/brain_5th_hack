# Reflex

第5世代 Sharp Brain 向けの `no_std` Rust crate。crate 名は `reflex`。
解析ツールとは独立し、アプリから使う描画・終了・PEG の ABI 定義をまとめる。
デフォルトでは外部依存・アロケーター・書き込み可能な static は持たない。
`application` featureで、追跡型アロケーターとPEGイベント型アプリの実行基盤を有効にできる。

## 利用

この monorepo 内のアプリからは path dependency で参照する。
crates.io への公開はしていない。

```toml
[dependencies]
reflex = { path = "../reflex" }
```

| モジュール | 内容                                                                     | 確認状況                                                                    |
| ---------- | ------------------------------------------------------------------------ | --------------------------------------------------------------------------- |
| `app`      | 起動コンテキスト、`exit`                                                 | V690_H1 の純正アプリと SharpLib で終了経路を確認                            |
| `display`  | 854×480 RGB565、回転座標、ピクセル書き込み、キャッシュ同期、表示更新     | 既存 Hello World の処理を移植、実機確認が必要                               |
| `peg`      | キー・タッチ・タイマーのメッセージ、描画用 ABI 型・vtable スロット       | Brain Hackers の公開サンプルに基づく、V690_H1 未照合                        |
| `runtime`  | malloc / free、TSTMR 時刻                                                | `experimental-runtime` が必要。malloc/free の公開入口を V690_H1 で確認      |
| `audio`    | PCM 再生、消費通知、補充、停止・状態取得                                 | `experimental-runtime` が必要。V690_H1 静的解析に基づく、実機未確認         |
| `peg::raw` | ウィンドウ生成、画面矩形、描画、フォーカス、タイマー、破棄の関数ポインタ | `experimental-peg` が必要。一部を純正アプリの公開入口へ照合済み、実機未確認 |

デバイスを操作する API は `arm` + `target_os = "none"` でのみ公開する。
音声 ABI と内部バッファ長の変更に関する制約は [音声解析メモ](../../docs/audio.md) を参照。
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

| `message.kind`                         | ペイロード                                                    |
| -------------------------------------- | ------------------------------------------------------------- |
| `PM_KEY` + `param == TOUCH_MOVE_PARAM` | タッチ移動。`data.point` が座標（PW-H1実機報告）              |
| `PM_KEY` / `PM_KEY_DOWN`               | `param` がキーコード、`data.ext_params[0]` が修飾・押下フラグ |
| `PM_KEY_UP`                            | キー解放の候補値。元資料でも未確定                            |
| `PM_LBUTTONDOWN` / `PM_LBUTTONUP`      | `data.point` がタッチ座標                                     |
| `PM_TIMER`                             | `param` がタイマー ID                                         |

`peg::key` に Enter、Escape、Space、Backspace、矢印、Shift と以下の特殊キーを定義。
PW-H1 の実機報告に基づく対応表。

| 定数                  | コード   | キー                                     |
| --------------------- | -------- | ---------------------------------------- |
| `PAGE_UP`             | `0x025c` | PageUp（二重上）、Shift で音量プラス     |
| `PAGE_DOWN`           | `0x025d` | PageDown（二重下）、Shift で音量マイナス |
| `HOME`                | `0x028a` | ホーム                                   |
| `JAPANESE_DICTIONARY` | `0x028b` | 国語/漢字                                |
| `ENGLISH_DICTIONARY`  | `0x028c` | 英和/和英                                |
| `MY_DICTIONARY`       | `0x028d` | My辞書                                   |
| `HISTORY`             | `0x028e` | 履歴                                     |
| `SELECT_FROM_LIST`    | `0x028f` | 一覧から選ぶ                             |
| `AUDIO`               | `0x0294` | 音声                                     |
| `INPUT_MODE`          | `0x0295` | あA1/記号                                |

音量操作は `PAGE_UP` / `PAGE_DOWN` と `KMOD_SHIFT` の組み合わせとして扱う。
PW-H1 の実機報告に合わせ、上は `0x25e`、左は `0x260` とする（参考サンプルのラベルを訂正）。
`KMOD_SHIFT`、`KFLAG_PRESS`、`KFLAG_HELD` はメッセージ内のフラグ。
`KFLAG_HELD = 0xc00` は複数ビットなので、判定は `(flags & KFLAG_HELD) == KFLAG_HELD`。
`PM_KEY` は先に `param == TOUCH_MOVE_PARAM`（`0x8000`）を確認し、タッチ移動をキーと区別する。
移動データは下位16bitがX、上位16bitがY。`0x01d0030e` は X=782、Y=464。
union の読み取りはイベント種別と param を確認してから `unsafe` で行う。

PEG 関数が必要なアプリだけ次を指定する。

```toml
reflex = { path = "../reflex", features = ["experimental-peg"] }
```

`peg::raw` は固定アドレスを C ABI で呼ぶ `unsafe fn` を公開する。
`dec_window_ctor`、`rect_set`、`screen_rect`、`adjust_client`、
`win_begin_draw`、`win_base_draw`、`get_screen`、`blit`、`set_current`、
`set_timer`、`kill_timer`、`destroy` が対象。タイマーの周期はミリ秒ではなく PEG tick。
各アドレス・ABI を使用中のファームウェアで照合してから呼び出す。
`blit` は PEG の描画開始・Bitmap 転送・描画終了をまとめたもの。
ホストの描画と共存するウィンドウでは、直接フレームバッファを書き換えずこちらを使う。

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

タイマー・ウィンドウ破棄は純正 PCConnect の公開入口に合わせ、フォーカス設定も
純正のインライン処理を使用する。照合箇所は [Doom の解析メモ](../../docs/doom.md#reflex-の公開入口の照合) を参照。

元資料の終了アドレス `0x840e7a48` は採用せず、V690_H1 で確認した公開入口
`0x8401013c` を使う。PEG は固定の公開アドレスとオブジェクト配置を使うため、版の違いに弱い。

```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p reflex --all-features --target armv7a-none-eabi -- -D warnings
make hello-world
```

ARM ビルド時に起動コンテキスト・メッセージ・Bitmap のサイズとオフセットを
コンパイル時 assertion で検査する。実機のイベント配送や描画動作までは検査できない。

## 命名

腕（ARM）に熱湯とかをかけると脳（Brain）で意識する前に動かすのを逃避反射（Withdrawal Reflex）というらしいので、それのReflexの部分。

## `#[reflex::main]` と alloc

```toml
reflex = { path = "../reflex", features = ["application", "panic-qr"] }
```

```rust,ignore
#![no_std]
#![no_main]
extern crate alloc;

#[reflex::main]
fn main() -> Calculator {
    Calculator::new()
}

impl reflex::Application for Calculator {
    fn event(&mut self, event: reflex::Event) -> reflex::Action {
        // 入力を処理し、None / Redraw / Exitを返す。
        reflex::Action::Redraw
    }
    fn draw(&self, pixels: &mut [u16]) {
        // 854×480、行優先のRGB565を描画する。
    }
}
```

マクロは引数なしの同期関数へ付ける。戻り値は `Application + 'static`。
`fn main()` と `unsafe fn main()` の両方を受け付ける。
unsafe版は起動コードのunsafeブロックから呼ばれる。関数の安全条件が起動環境で満たされることはアプリ側で保証する。
mainから戻ることは終了を意味しない。reflexが状態と描画バッファを所有して
PEGへ戻り、キー・タッチを配送する。終了は `Action::Exit` を返す。
`event`の `Action::None` と未知のPEGメッセージは元のウィンドウハンドラーへ渡す。
`Event::Key`は生のキーコード・押下/解放・フラグを保持するため、
PM_KEYとPM_KEY_DOWNの重複除去はアプリ側で行う。

起動コードは `.data` / `.bss` を初期化する。リンクには `crates/reflex/link.x` を使う。
コードの先頭は `0x90000000`、RAM初期値は `0x90500000`。
マクロはグローバルアロケーターとpanic handlerも定義するため、アプリ側で重複定義しない。
実装例は [関数電卓](../brain-calculator/src/main.rs)、ビルド例は `make calculator`。

Doom・Inspector・HelloWorldも同じ `#[reflex::main]` / `Application` を使う。
必要に応じて次のフックを実装できる。呼び出しは同じPEGスレッドで直列化される。

- `mounted(Window)`: ウィンドウ登録後に呼ぶ。`raw` と `presentation` を使い、タイマーや音声通知先を設定する。生APIの安全条件は引き続き呼び出し側で満たす。
- `raw_event(&peg::Message)`: 型付き入力より先に呼ぶ。`None` なら通常の `event` 配送へ進み、`Some(Action::None)` なら描画せず消費する。タイマー・音声通知やInspectorの生キー種別の記録に使う。
- `Action::RedrawAndBubble`: 描画後、元のメッセージをホストへ渡す。Inspectorのウィンドウクリップ用ジェスチャーを保つ。
- `before_draw(Rect)`: ホストからの再描画も含め、描画直前に呼ぶ。Inspectorの矩形計測に使う。
- `can_exit()`: Exit要求やホストの破棄通知で呼ぶ。音声停止・タイマー停止などを行い、成功時だけ `true` を返す。`false` ならアプリを解放せず再描画し、次の終了要求を待つ。

終了を保留できるリソースの停止は `can_exit` で行い、その後の通常の所有値の解放はDropに任せる。
フック内では生の `app::exit` を呼ばず、Reflexに終了を任せる。

正常終了では、受付停止・フォーカス復元・元vtable復元・ウィンドウ破棄を行い、
アプリ状態と描画バッファをdropしてから、Rustアロケーターに残る確保領域を一括解放する。
最後に元の起動コンテキストで終了通知する。所有値のDropで外部リソースも解放すること。
アプリは独自の外部コールバックを登録したまま終了してはいけない。
一括解放はDropを実行せず、`runtime::alloc`の直接呼び出しは追跡しない。
既存の低レベル `app::exit` は自動解放を行わない。

`alloc` featureだけで `allocator::SystemAllocator` を使うこともできる。
これは単一コアのPEGスレッド向けで、割り込みハンドラーや複数コアからは使用しない。
ヘッダーは元ポインタと前後のリンクの3ポインタ分、加えて整列の余白を消費する。
`release_all` は全所有値の破棄とコールバック停止後に限るunsafe API。
起動用staticと割り込みの初期化は、手動登録する側が保証する。

`panic-screen`はヒープなしで文字を描画してspinし、`panic-qr`はさらにQRを表示する。
両方無効なら従来どおりspinだけを行う。panic時は一括解放・終了通知を行わない。
診断は最大192 UTF-8バイトで、切り詰めを `...` として表示する。
QRはVersion 10・誤り訂正M以下、固定バッファと4モジュールの余白で生成する。
ASCII以外の画面文字は `?` で表示し、QRにはUTF-8を保持する。
画面保持、watchdog、実機でのQR読取、PEG破棄・再入は実機検証が必要。

`text`（`panic-screen`・`panic-qr`を含む）の標準フォントは `ascii-6x13.bin`。
ASCII 95文字を1行1バイトで保持し、サイズは **1,235バイト（約1.21KiB）**。
各文字は `assets/ascii-6x13.glyphs` に手描きの5列パターンとして定義し、右端1列を字間にする。大文字は7行、小文字は5行を基本に、上昇部・下降部を個別に描いている。
6×13ドットを2倍で表示し、既存の12×26セルと `scale` の意味を維持する。
描画は前景色・背景色の選択だけで、濃淡のブレンド計算は行わない。

`font-antialias` featureを有効にすると従来の `ascii.gray` に切り替わる。
こちらは95×12×26の8bit濃淡データで **29,640バイト（約28.95KiB）**。
どちらも静的な読み取り専用データとして埋め込み、ヒープへ展開しない。
Cargoのfeatureは統合されるため、同じビルド内の依存先が `font-antialias` を有効にした場合も滑らかな版になる。

標準フォントは `ruby scripts/reflex-bitmap-font.rb` で手描きパターンをそのままビット列へ変換する。画像処理・縮小・閾値処理は行わない。
濃淡版だけがDejaVu Sans Mono由来で、再生成は
`ruby scripts/doom-help-font.rb /path/to/DejaVuSansMono.ttf`。
Doom独自のヘルプ用アトラスは、大文字小文字を読みやすく表示するため従来の濃淡版を維持する。
アプリ配布時は `assets/FONT-LICENSE.txt` を同梱する。
