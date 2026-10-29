# reflex CLI

[Usage](https://usage.jdx.dev/rust/quickstart) のRust deriveで定義した、Brainアプリのビルド・配置ツール。
Cargo workspace内で実行する。別の場所から使う場合は `--manifest-path` を指定する。

```sh
cargo install --path crates/reflex-cli --locked

reflex build brain-inspector
make doom-fetch
reflex build brain-doom

# ビルドしてSDカードのAPP/Inspectorへ配置
reflex deploy brain-inspector /media/user/SDCARD

# WSLからWindowsのドライブへ配置
reflex deploy brain-inspector I:/
```

Cargoパッケージ名に加え、`inspector`、`doom`、`calculator`、`hello-world` も使える。
未インストールでも `cargo run -p reflex-cli -- build inspector` の形式で実行できる。
MakefileのアプリビルドはこのCLIへの互換窓口。外部入力の取得（`make doom-fetch`）とexample previewの実行は、従来どおりMakefile・スクリプトに残している。CLIからダウンロードやプレビューは行わない。

- `build`: `cargo rustc`でARM ELFを生成し、`llvm-objcopy-20`でAppMain.binへ変換。
  `index.din`とライセンスなどの同梱ファイルをアプリ用フォルダーに配置する。
- `deploy`: `build`の後、指定した既存ディレクトリ内の `APP/<name>` へコピーする。
  同名ファイルは上書きするが、他のアプリや配置先だけにあるファイルは削除しない。
  コピー中の失敗では部分的な更新が残るため、エラーを解消して再実行する。

`--target-dir`（または `BUILD_DIR`）で成果物ディレクトリ、`--objcopy`（または `OBJCOPY`）で変換ツールを指定できる。
相対のtarget-dirはworkspaceルート基準。未指定ならCargo metadataのtarget_directoryを使う。
`CARGO`でCargo実行ファイルを指定できる。ARMターゲット・LLVM・Doom取得時のRuby/curlは従来どおり必要。

## WSLからの配置

WSL上で配置先が `I:/`、`I:\`、UNCパスなどのWindows形式なら、
ビルド成果物の絶対パスを `wslpath -w` で変換し、PATH上の `powershell.exe` からコピーする。
Windows側からWSLの共有パスを読み取るため、共有用の一時ディレクトリや特定ドライブのLinuxマウントは不要。
WSLのWindows相互運用とPATH設定が有効である必要がある。
パスはPowerShellの文字列リテラルとしてエスケープし、読み取りには `-LiteralPath` を使う。
通常のLinuxパスとWindowsネイティブ実行時はRustのファイルAPIを使用する。
Windows形式のパスをWSL以外のLinuxで指定した場合はエラーにする。

参考: [WSL相互運用](https://learn.microsoft.com/en-us/windows/wsl/interop)、
[Copy-Item](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/copy-item)。

## アプリの登録

各アプリのCargo.tomlに設定する。`device` featureと、パッケージ名と同名のバイナリをビルドする。
`linker`と`assets`はそのCargo.tomlのディレクトリ基準。同梱ファイルはファイル名を保ってアプリ直下にコピーする。

```toml
[package.metadata.reflex]
name = "MyApp"
output = "my-app"
linker = "../reflex/link.x"
link-args = ["--no-undefined"]
assets = ["../reflex/assets/FONT-LICENSE.txt"]
```

出力は `<target-dir>/my-app/APP/MyApp/`。
外部入力がある場合はDoomの `package.metadata.reflex.inputs` を参照。
入力ディレクトリはtarget-dir基準で、`env`に指定した環境変数へその絶対パスを渡す。
`inputs.assets`は入力ディレクトリ基準の同梱ファイル。

## Usage定義

引数解析・ヘルプ・Usage specは同じRust宣言から生成する。

```sh
reflex --help
reflex deploy --help
reflex __usage_spec__ > target/reflex.usage.kdl
# Usage CLIをインストールしている場合
usage generate markdown --file target/reflex.usage.kdl --multi --out-dir target/reflex-docs
```
