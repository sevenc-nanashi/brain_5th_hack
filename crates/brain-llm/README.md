# LLM / 英日翻訳

PW-H1本体で動く翻訳アプリ。既定は **ElanMT tiny en-ja（15.6Mパラメータ）**。
モデル一覧には英文生成の動作確認用TinyStories 260Kも残している。
Qwenは実機で約300秒/tokenだったため、英→日専用の小型モデルへ置き換えた。

```sh
cargo run -p reflex-cli -- deploy llm J:/ --build
```

初回ビルドはPCで元モデル約31MBとトークナイザーを自動取得し、SHA-256を検証する。
Rubyで行単位のINT8重みとSentencePieceのunigram辞書へ変換する。
元ファイルはPCのキャッシュに残し、配布物には変換結果だけを含める。
`make llm` も同じ経路。通常のビルドにはRubyとcurlが必要。

- `elan.bin`：16,536,516バイト
- `elan.tok`：649,650バイト
- アプリ・TinyStoriesを含む配布フォルダー全体：約19MB

配布フォルダー `target/llm/APP/LLM/` 全体をコピーすること。
以前のQwenファイルは生成済み配布フォルダーから除去する。
SD上の既存ファイルはdeployで消さないため、以前の
`APP/LLM/qwen2.5-0.5b-instruct-q4_0.gguf` と `qwen.meta` は手動で削除してよい。

## 操作

- 一覧の上下・決定で選択／読み込み。初期選択は英日翻訳。
- 英数字入力（最大128文字）。Shiftで大文字、Backspaceで削除、Shift＋Backspaceで全消去。
- 決定で翻訳開始。初期入力は `Hello.`。
- 生成中のEscで停止。停止中・読み込み中のEscでモデル一覧へ戻る。一覧のEscで終了。
- 上下で出力をスクロール。生成中は末尾へ追従。

切り替え時は前のモデル・作業領域・ファイルを解放し、入力と出力をリセットする。
ファイル欠落・破損・メモリ不足・I/O失敗はエラー表示し、Escで一覧へ戻れる。
本体でも変換済みファイルのSHA-256を分割検証する。

## 軽量化

4層encoder＋4層decoder、隠れ幅256、語彙32,001のMarian型モデルを使用する。
行列の重みをINT8に量子化し、入力ベクトルも動的にINT8へ量子化して、
内積を整数で計算する。スケール・バイアス・LayerNorm・attention等はFP32。
FPUやNEONには依存しない。

重みはRAMへ一度読み込み、翻訳中はSDを読まない。
encoder出力とcross-attentionのK/Vは文ごとに一度計算し、decoderのK/Vも再利用する。
出力行列は256行ずつに分割し、タイマー処理は約8msを目安にアプリへ制御を戻す。
1ブロックの計算中はこの目安を超える場合があり、停止操作はブロック完了後に処理する。

推論用メモリは約19MB（約18.9MiB）。別に画面バッファ等が必要。
`WORK` は推論用確保量であり、総RAM・空きRAMではない。
最大入力128トークン・出力128トークン、greedy生成。履歴は保持しない。
TinyStoriesは従来通り最大512文脈／128生成トークン。

入力はASCIIのみ。出力はかな・CJK漢字・全角記号を同梱の16×16ビットマップで表示し、
未収録文字は`?`にする。日本語入力・日→英翻訳には未対応。
常駐・スリープ抑止は使用しない。

**新モデルの実機速度・操作は未確認。** 小さいモデルかつ量子化・greedy生成なので、
翻訳精度は限定的。元モデルも高精度を必要とする用途には不向きと明記している。

## ホストでの確認

```sh
cargo run --release -p brain-llm --example translate -- \
  target/llm/downloads/elan.bin target/llm/downloads/elan.tok \
  'This is a small computer.'
```

上流Transformers 4.47.1のFP32・greedy生成と短文5件で比較した。
`Hello.`／`I like reading books.`／`Where is the train station?`／
`Please open the window.` は訳文が一致。
`This is a small computer.` は上流「これは小さなコンピューターです。」に対し、
INT8版「これは小型のコンピューターです。」となる。量子化で出力は変わりうる。
ホストPCではこれらの全文生成は約20〜30msだったが、Brainの速度を示す値ではない。

ASCII入力206件のトークン列が公式SentencePiece 0.2.0と一致。
メタデータ切断の拒否、空入力・日本語入力・長すぎる入力の拒否、
繰り返し翻訳、中断後の別文への再開を一時検証で確認。
ARMビルド、ホスト／ARM Clippy、自動取得から一時フォルダーへのdeployを確認。

```sh
# TinyStoriesの参照出力比較用
cargo run -p brain-llm --example generate -- \
  target/llm/downloads/stories260K.bin target/llm/downloads/tok512.bin
# 日本語フォントの再生成（FreeType開発用ライブラリ、cc、pkg-configが必要）
ruby scripts/llm-japanese-font.rb /path/to/NotoSansCJK-Regular.ttc
```

## 出典・ライセンス

- [ElanMT tiny en-ja](https://huggingface.co/Mitsua/elan-mt-tiny-en-ja/tree/b6d2a119e8742c20a0007420a0d9209b6a9bb753)：ELAN MITSUA Project / Abstract Engine、CC BY-SA 4.0。変換済みモデル・辞書も同ライセンス。変更内容と帰属は`ELAN-MODEL.txt`、条文は`LICENSE-ELAN.txt`を同梱する。
- [Transformers Marian](https://github.com/huggingface/transformers/blob/v4.47.1/src/transformers/models/marian/modeling_marian.py)：演算仕様と翻訳結果の照合先。
- [SentencePiece](https://github.com/google/sentencepiece)：unigramトークナイザー。
- [llama2.c](https://github.com/karpathy/llama2.c)：TinyStories推論の移植元（MIT、LICENSE-LLAMA2.txt）。
- [TinyStories 260K](https://huggingface.co/karpathy/tinyllamas/tree/0bd21da7698eaf29a0d7de3992de8a46ef624add/stories260K)（MIT）。
- [Noto CJK](https://github.com/notofonts/noto-cjk)（SIL OFL 1.1、FONT-JAPANESE-LICENSE.txt）。
