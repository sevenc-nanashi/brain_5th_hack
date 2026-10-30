# PocketJS / MicroTS

[PocketJS](https://github.com/pocket-nexus/pocketjs) のMicroTSで、SolidのTSXビューと
TypeScriptのカウンター処理をRustへ事前コンパイルするサンプル。
実機ではPocketJSのUIコアとソフトウェアラスタライザーが動き、Reflexが入力と画面出力を担当する。
JavaScriptインタープリターは使用しない。

## ビルドと操作

```sh
make pocketjs
# または
cargo run -p reflex-cli -- build pocketjs
```

`target/pocketjs/APP/PocketJS/` をSDカードの `APP/PocketJS/` へコピーする。
`AppMain.bin`、`index.din`、ライセンスファイルをまとめて配置する。
初回ビルドにはGitHubとcrates.ioへの接続が必要。
生成済みRust・スタイル・フォントを同梱しているため、通常のビルドにBunは不要。

- 決定（Enter）: カウンターを1増やす。9999の次は0。
- R: 0に戻す。
- Esc: 終了。

キー状態をMicroTSへ渡し、押しっぱなしの重複通知による加算を防ぐ。
タッチ操作、ファイル読み込み、ネットワークはこのサンプルには含めない。
画面は左側800×480を使い、Reflexの854×480 RGB565バッファへ描画する。

## TSX / TypeScriptの編集

ビューは `app/App.tsx`、状態とイベント処理は `app/App.ts`。
編集後は、依存と同じ固定リビジョンの公式コンパイラーで再生成する。

```sh
bun install --frozen-lockfile
bun run pocketjs:generate
cargo fmt --all
make pocketjs
```

ルートの `package.json` と `bun.lock` でコンパイラー・Solid・TypeScriptを管理する。
`bun run pocketjs:build` は再生成と実機向けビルド、
`bun run pocketjs:preview` は再生成とホストプレビューを続けて実行する。
手元の公式ソースを使う場合は `bun run pocketjs:generate <checkoutのパス>` で指定できる。
その場合もCargo依存と同じリビジョンであることを検査する。

生成先は `src/generated/` と `assets/`。生成されたRustを直接編集しない。
フォントはInterのASCII文字を24px・54pxで焼き込む。
文字サイズや文字集合を変更するときは生成スクリプトも合わせて変更する。

## ホストプレビュー

```sh
make pocketjs-preview
cargo clippy -p brain-pocketjs --all-targets -- -D warnings
cargo clippy -p brain-pocketjs --features device --target armv7a-none-eabi -- -D warnings
```

`target/pocketjs-preview/` に初期状態・加算後・リセット後のPPM画像を出力する。
同じPocketJSコアと生成済みアプリを使い、Reflexのキーイベントで操作する。
ARM向けビルドとホスト描画を確認。実機の起動・入力・描画速度は未確認。
