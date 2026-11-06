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
`fs` は読み取り専用の `File` / `Directory` を提供する（`experimental-runtime`）。
`storage` featureでは、アプリ用の `storage::Store` を有効にする。保存先の親フォルダーを作成し、
チェックサム付きJSONを2世代で保持する。書き込み失敗時は直前の正常な世代を残す。
TIC-80の履歴・pmemとDoomのセーブ・設定で共有する。ホストでは通常のファイルを使って確認できる。
V690_H1 の静的解析に基づき、実機未確認。ABI の根拠と制限は [ファイル操作 API](../../docs/filesystem.md) を参照。
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
- [Hello World の解析メモ](../brain-hello-world/README.md): 描画アドレスと純正アプリの終了経路。

タイマー・ウィンドウ破棄は純正 PCConnect の公開入口に合わせ、フォーカス設定も
純正のインライン処理を使用する。照合箇所は [Doom の解析メモ](../brain-doom/README.md#reflex-の公開入口の照合) を参照。

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

起動コードは `.data` / `.bss` を初期化する。リンクには `reflex-cli` に内蔵した `link.x` を使う。
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

## 操作バー（実験的）

`operation_bar::Config` と `operation_bar::raw` は V690_H1 の右端操作バー用。
`experimental-peg` を有効にした ARM 実機で使用する。
`create` / `destroy` / `control` / `suspend_updates` / `add_item` を公開する。
生成時の第2・第4引数は用途未確定のため `unknown1` / `unknown3` とし、
PCConnect・SetMenu・pvss・DicMainで確認した値はどちらも0。
`Config::pcconnect(slots)` は純正の設定値を使う。未確定フィールドは `unknown_*`。
項目登録は未確定の構造体を受けるunsafe APIで、全形式の安全なラッパーは提供しない。
呼び出しはPEGイベント処理内で行い、通知先ウィンドウより先にバーを破棄する。
Inspectorの一覧キー → Bで生成・表示・通知を試せる。実機検証は未完了。
`operation_bar::Bitmap` は自作RGB565画像の入力記述子。flags、bppの順で、
既存の `peg::Bitmap` とは別の型にしている。項目の `+0x20` / `+0x40` に
通常／押下Bitmapへのポインタを渡す。Inspectorの `bar_image` が使用例。

`operation_bar::image` は V690_H1 の52×52純正画像IDを `u32` 定数で公開する。
例：`image::HOME`（0x0460）、`image::HOME_PRESSED`（0x0461）、
`image::SETTINGS`（0x0610）、`image::SETTINGS_PRESSED`（0x0611）。
`EDNewRes_Color.bin` の共通側218件と辞書側55件の計273件を収録。明色版は `LIGHT_HOME` / `LIGHT_HOME_PRESSED` などの `LIGHT_` 接頭辞で参照する。
`image::ALL` は `(名前, 画像ID, 押下時画像ID)` の一覧で、Inspectorの選択欄にも使用する。
通常画像には対応する押下画像を指定し、押下・無効・選択色などの単独画像は同じIDを指定する。
定数名は画像の内容を表し、キー送信や本体機能の実行、無効状態の設定は行わない。

### 二段展開と明色テーマの解析

V690_H1のSharpLibと、更新パッケージ001のDicMainで静的に確認した。
以下のオフセットは各バイナリーの先頭基準。実機での再現は未確認。

- `create` の第2・第4引数はSharpLib `0x111428` / `0x11142c` で
  ハンドルの `+0x10` / `+0x14` に保存される。今回追跡した開閉・配色処理はこれらを参照せず、
  DicMainでも両方0のまま。用途の断定はせず `unknown1` / `unknown3` を維持する。
- 展開ボタンは項目記述子先頭の `(flags & 0xc000) == 0x4000`。
  SharpLib `0x111eb4`〜`0x111ef8` が内部フラグbit20へ変換する。
  DicMain `0x370a64` / `0x370b94` では `0x4121` を使い、前者は
  `0x370afc` で縦横画像切替の `0x10` も加えて `0x4131` として登録する。
  開く画像は `0470/0471`、閉じる画像は第2状態の `0638/0639`。
  記述子 `+0xa4` の第2状態画像があると、SharpLib `0x111efc`〜`0x111f24` が状態数を2へ増やす。
- 1グループの枠を超えて `add_item` すると、SharpLib `0x112658`〜`0x112678` が
  同じ枠数のグループを追加する。展開ボタンを押すと `0x110d50`〜`0x110e98` が
  ルートのグループ数を数え、幅を `54 × グループ数` に変更して横へ配置する。
  展開中のボタン操作は `0x110c08`〜`0x110cd8` を通り、幅54へ戻す。
  InspectorではOキーの2列モードで「登録数＝1列の枠数×2」とし、先頭に開閉ボタンを登録する。
  1列モードで枠数を増やすだけでは展開にはならない。
  展開時も矩形領域は合計60枠分であり、「60枠×複数グループ」を許容する意味ではない。
- 明色テーマは `create` の第3引数のConfigと、画像IDの組み合わせ。
  DicMain `0x36ff8c`〜`0x36ffc4` は `0x85004fbc + 0x74` の値に応じ、
  `background_color` を暗色 `0x001f1e22`／明色 `0x00dcdcdc`、
  `boundary_color` を白 `0x00ffffff`／黒 `0` に切り替える。
  SharpLib `0x10fee8`〜`0x10ff24` がConfigを保持し、`0x1104f4`〜`0x110554` が描画に使う。
  色は `0x00BBGGRR`。背景ブラシが非nullの場合は色よりブラシが優先される。
- 明色画像は `EDNewRes_Color.bin` 内の `color_res_dic.bin` にあり、
  addrテーブルのインデックスは `画像ID - 0x8000`。抽出画像でも絵柄を確認した。
  共通画像 `color_res_lib.bin` の218件とは別に存在する。

| 絵柄 | 暗色の通常／押下ID | 明色の通常／押下ID |
| --- | --- | --- |
| ホーム | `0460/0461` | `89B1/89B2` |
| 上 | `0468/0469` | `89B3/89B4` |
| 下 | `046A/046B` | `89B5/89B6` |
| 右 | `046C/046D` | `89B7/89B8` |
| 左 | `046E/046F` | `89B9/89BA` |
| ツール | `03F1/03F2` | `89BB/89BC` |
| 決定 | `0462/0463` | `89BD/89BE` |
| 戻る | `0464/0465` | `89BF/89C0` |
| 縦横切替 | `017C/017D` | `89C1/89C2` |
| 電源 | `01A2/01A3` | `89C3/89C4` |
| 開く | `0470/0471` | `89C5/89C6` |

DicMainの画像切替は `0x370088`〜`0x3701c8`、`0x3703c0`〜`0x3705f4`、
`0x370a04`〜`0x370b18`、`0x371548`〜`0x37160c` を参照。
背景色だけを変更しても、暗色画像に描かれた背景は変わらない。

### 明色ボタンの追加調査

`color_res_dic_addr.bin` の3,699エントリーを走査し、52×52画像を55件確認した。
うち12件（手書き・50音・abcの各4状態）は共通側とRGB565ピクセルが完全一致する。
残る43件は明色背景などの別画像。`89C5` と `89C7` の通常画像は同一だが、
押下画像 `89C6` と `89C8` は異なる。IDだけをまとめて置換しないこと。

先の表に加え、以下の通常／押下ペアを抽出画像で確認した。
履歴・単語帳・マーカーテスト・設定はDicMainの条件分岐でも暗色との切替を確認した
（`0x37175c`〜`0x371788`、`0x371940`〜`0x37196c`、
`0x371b24`〜`0x371b50`、`0x3725c0`〜`0x3725f4`）。

| 絵柄 | 暗色の通常／押下ID | 明色の通常／押下ID |
| --- | --- | --- |
| 開く（別種） | `0472/0473` | `89C7/89C8` |
| 四方向移動 | `0466/0467` | `8A82/8A83` |
| 単語帳 | `00EF/00F0` | `8A84/8A85` |
| 履歴 | `00EB/00EC` | `8A86/8A87` |
| マーカーテスト | `00F1/00F2` | `8A88/8A89` |
| 戻す矢印 | `0476/0477` | `8A8A/8A8B` |
| 設定 | `0610/0611` | `8CAF/8CB0` |
| 閉じる（横向き） | `0638/0639` | `8E4D/8E4E` |
| 開く（縦向き） | `078A/078B` | `8E6C/8E6D` |
| 閉じる（縦向き） | `078C/078D` | `8E6E/8E6F` |

空白の明色画像は `89C9`（暗色 `04A0`）。明色版の文字・記号は黒、
通常背景は薄灰色、押下背景は橙色。押下色も自動着色ではなく画像に含まれる。

入力切替ボタンの対応は以下。並びは通常→無効→選択→押下であり、
**この範囲では通常ID+1が押下画像ではない**。状態名は既存の絵柄分類に合わせたもの。

| 絵柄 | 辞書側：通常／無効／選択／押下 | 同一ピクセルの共通側ID |
| --- | --- | --- |
| 手書き | `8A76 / 8A77 / 8A78 / 8A79` | `047B / 047D / 04C0 / 047C` |
| 50音 | `8A7A / 8A7B / 8A7C / 8A7D` | `047E / 0480 / 04C1 / 047F` |
| abc | `8A7E / 8A7F / 8A80 / 8A81` | `0481 / 0483 / 04C2 / 0482` |

文字サイズ・拡大縮小・削除など、共通側にある全ボタンの明色版が揃っているわけではない。
今回の55件は52×52に限定した一覧で、別サイズ・自作描画による明色ボタンは対象外。
Inspectorの候補と定数に明色版55件も追加済み。InspectorのLキーで背景色と対応画像をまとめて切り替え、左右キーでは個別選択できる。
