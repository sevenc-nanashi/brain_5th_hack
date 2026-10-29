# 関数電卓

<!-- impeccable:product-schema 1 -->

## Platform

PW-H1の854×480 RGB565バッファの左側800×480に描画するネイティブ組み込みアプリ。Web/iOS/Androidではない。

## Users and purpose

電子辞書上で関数計算を行う利用者、およびreflexのalloc・イベント型main・panic表示を使う開発者。
基本の関数電卓を、タッチと物理キーの両方で操作する。

## Constraints

Rustのno_std + alloc、V690_H1のPEG公開入口を使用する。
英数字中心、履歴は別画面、入力512バイト、履歴32件。外部ファイルやネットワークは使用しない。
正常終了で所有値を破棄し残存確保領域を回収する。panic時は文字とQRを表示して停止する。

## Evidence

計算規則と実機確認項目は../../docs/calculator.mdに記載する。
ホストの描画プレビューは実機の表示・操作確認を代替しない。
既存アプリの視覚設計を変更するものではない。
