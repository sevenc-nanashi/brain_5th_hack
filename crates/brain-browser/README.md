# Browser

NetSurf 3.11をReflexへ移植した、PW-H1 / V690_H1用の実験的ブラウザ。
起動ページは `http://abehiroshi.la.coocan.jp/`。
PCがHTTP(S)を取得し、Brain側のNetSurfがHTML/CSS・フレーム・画像を解析して描画する。
PCでレンダリングした画像を送る方式ではない。

本文は美咲ゴシックの8×8ドットを整数倍で描画する。BDFのUnicode対応表・半角幅を維持し、
文字幅計測と描画で同じ字形情報を使う。Shift_JISなどの変換はlibiconvが担当する。

## ビルド

Linux x86_64、Rustの `armv7a-none-eabi` ターゲット、Ruby、Clang/LLVM 20、
GNU make、Perl、pkg-config、curl、tar、unzip、dpkg-debが必要。
Newlibとgperfは取得スクリプトが作業ディレクトリへ展開する。システムへのインストールは行わない。

```sh
make browser
```

`make browser` と `make browser-preview` は依存ソースを先に取得する。
Cargoまたはビルドスクリプトを直接実行する場合は、先に `make browser-fetch` を実行する。
取得物はバージョンとSHA-256を固定。出力は `target/browser/APP/Browser/`。
`BUILD_DIR` を変更する場合、取得とビルドの両方で同じ値を指定する。
Cargo自身の出力先も変更したい場合は `CARGO_TARGET_DIR` を設定する。

```sh
cargo run -p reflex-cli -- deploy browser /mnt/brain
sync
sudo umount /mnt/brain
```

`AppMain.bin` と `index.din`、`LICENSES.txt` を一緒に配置する。

## 実機で開く

1. Brainの保存領域をアンマウントし、純正PCConnectを終了する。
2. 追加アプリ **Browser** を起動し、Enterを押す。
3. DRMモードでUSBが再接続される。WSLでは必要に応じてUSBを再アタッチする。
4. PCで対応する `/dev/sgN` を確認し、中継プログラムを起動する。

```sh
lsusb -d 04dd:92e1
ls -l /sys/class/scsi_generic/*/device/block
sudo ruby scripts/usb-browser.rb /dev/sg4
```

`sg4` は例。中継側はVID/PID、DRM revision `0103`、4096×512 bytesの容量、
未マウント状態を確認してから通信する。Explorer用受信器と同時には起動しない。
TLSはPC側で証明書を検証する。HTTPからHTTPSへのリダイレクトもNetSurfへ通知するため、
フレームや画像の相対URLは遷移先を基準に解決される。

- タップ：リンクを開く。
- 上下 / PageUp・PageDown：最後に触れた位置のフレームをスクロール。初期位置は右側。
- Backspace：履歴を戻る。
- Esc：USBを切断して終了。

エラーやPC側の再起動後は、Escで終了してBrowserと中継を起動し直す。
通常のファイル転送へ戻すには純正PCConnectを起動する。

## PC上のプレビュー

```sh
make browser-preview
```

同じNetSurfフロントエンド・美咲フォント・HTTP中継処理で、854×448のPPMを
`target/browser-preview.ppm` へ出力する。実機画面の下32pxは状態表示用。
このプレビューはUSBとARM実行環境を通さない。

別のURLも指定できる。

```sh
ruby scripts/browser-preview.rb target/netsurf-inputs target/preview.ppm https://example.com/
```

## 範囲と制限

- GET、HTML/CSS、フレーム、JPEG/GIF/BMP、リンク移動を対象とする。
- 起動URLは固定。URL入力欄、POST、Cookieによるログイン、ダウンロード保存は未実装。
- JavaScript、PNG/WebP/SVG、動画は無効。ベジェ曲線によるパス描画も未対応。
- 1応答の展開後本文は最大2 MiB、1画像は最大1,048,576画素、URLは2048 bytesまで。
  リダイレクト先URLとContent-Typeヘッダーはそれぞれ424 bytesまで。
- Webフォントや絵文字は使わない。美咲にない字形は枠で示す。
- 字形の太字・斜体、破線の線種、禁則処理などの細かい組版は未対応。
- メモリキャッシュは2 MiBを目安とするが、DOMや展開画像を含む全使用量の上限ではない。
- NetSurf処理は512 KiBの専用スタックで実行し、アロケーションはReflexで追跡する。
  終了時はUSB停止後にNetSurfを破棄する。

上流の変更はビルド時に適用する2点だけ：HTTP(S)用fetcherの登録と、curl無効時のヘッダー依存の除去。
OSに依存するファイルfetcherはビルド対象から外す。C標準ライブラリのファイル・プロセス操作は
成功したふりをせず、未対応エラーを返す。

## USBプロトコル BRNWEB01

[既存のDRM通信](../../docs/usb.md)のデータペイロードに、固定468-byteのフレームを1個載せる。
整数はlittle-endian u32。一度に未完了の要求は1件。PCが要求を送り、Brainが応答する。

| offset | フィールド |
| --- | --- |
| 0 | `BRNWEB01`（8 bytes） |
| 8 | PCが生成するセッショントークン（8 bytes） |
| 16 | 通信番号。0から始め、応答ごとに1増える |
| 20 | NetSurfの取得ID。0は取得待ち |
| 24 | 種別 |
| 28 | 本文またはURL内の位置 |
| 32 | 総量。最初のpollではPCのUnix時刻（秒） |
| 36 | PC→BrainはHTTPステータス、応答では0=受理・1=取得キャンセル済み |
| 40 | データ長（0〜424） |
| 44 | データ。余りは0で埋める |

PC→Brainの種別は0=poll、1=ヘッダー、2=本文、3=完了、4=取得失敗、5=リダイレクト。
Brain→PCは10=URL、11=ACK。長いURLはpollの位置指定で分割取得する。
ヘッダー・本文・完了は同じ取得IDで送り、本文の位置は受理されたバイト数だけ進める。
完了時に総量を照合する。キャンセル済みならPCはその応答の転送を止める。

Brainはセッション・通信番号・長さ・位置を検査し、同一フレームを二重適用しない。
PCは前の応答を無視し、未送信bitが0ならfirmwareの保持パケット再送を要求する。
タイムアウト後の自動再接続や転送再開は行わない。

## 検証状況

ARM向けのリンクとAppMain.bin生成、PC上での実サイト取得・描画を確認。
プレビューではHTTP→HTTPS、Shift_JIS、日本語、左右フレーム、写真・背景画像を確認した。
ホスト上の一時的な自己チェックで、実装したUSBフレーム処理の分割受信・重複抑止・
URL分割・順序・長さ・完了サイズ・セッション検査を確認。USB firmwareとNetSurf呼び出しは
この自己チェックでは代替しており、実機通信の検証ではない。
実機でのBrowser起動、PC→Brainへの連続転送、速度・最大メモリ使用量は未確認。

## ライセンス

この移植部分とNetSurfはGPL-2.0-only。各依存ライブラリと美咲フォントの原文を
取得スクリプトが `LICENSES.txt` にまとめる。バイナリを配布する場合は、対応する
NetSurf・依存ライブラリのソース、移植コードとビルドスクリプトも提供する。

- [NetSurf 3.11](https://www.netsurf-browser.org/downloads/source/)
- [美咲フォント](https://littlelimit.net/misaki.htm)
