# brain-rom-dumper

コマンドはリポジトリのルートで実行する。

SDU の構造・チェックサムを検証し、更新段階ごとにファイルを取り出す。
検証対象は `SH_DIC_UPDATE_V690_H1.SDU`（136,808,651 bytes）。
これは更新パッケージの展開ツールであり、実機から ROM を読み出すツールではない。

```sh
# メタデータとファイル一覧を JSON で表示
cargo run -p brain-rom-dumper -- info '/mnt/z/SH_DIC_UPDATE_V690_H1(1).SDU'

# 新しいディレクトリへ展開
cargo run -p brain-rom-dumper -- extract '/mnt/z/SH_DIC_UPDATE_V690_H1(1).SDU' target/rom-dump
```

出力先は未作成のディレクトリを指定する。入力を検証してから書き出し、既存の出力先は拒否する。
書き込み中の I/O エラーでは部分的な出力が残るため、別の出力先で再実行する。
入力全体をメモリに読み込むため、入力サイズ以上の空きメモリが必要。

```text
target/rom-dump/
  manifest.json             # 段階、パス、オフセット、サイズ、操作種別など
  header.bin                # 元の SDU ヘッダー
  updater.bin               # 前半の生データ（名称は推定）
  boot.bin
  system.bin
  updates.bin               # 更新レコード領域の生データ
  packages/
    001/ROM/...             # 1.80 → 3.30
    002/ROM/...             # 3.30 → 3.40
```

サンプルには 2 パッケージ、84 レコードがあり、81 件のファイル本体を展開できる。
残り 3 件は本体のない操作レコードとして manifest に残す。同名ファイルも更新段階別に保持する。
前半領域の復号・逆アセンブル、内包ファイルの独自圧縮の展開、実機への更新適用・再パックは行わない。
更新に含まれないファイルは復元できないため、展開結果は完全な ROM イメージではない。

解析結果のフィールド定義と未確定事項は [docs/sdu-format.md](../../docs/sdu-format.md) を参照。
実データと展開物はリポジトリに含めず、`target/` 配下に保存する。
