# Explorer

<!-- impeccable:product-schema 1 -->

## Platform

第5世代 Brain のネイティブ追加アプリ。Rust / Reflex を使用する。
画面の操作領域は800×480、描画バッファは854×480。

## Purpose

実機でフォルダーを移動し、ファイル内容をテキスト／16進表示で閲覧する。
書き込み・削除は行わない。

## Constraints

既存アプリの共通フォントとキー・タッチ操作を利用する。
ファイル操作はV690_H1の公開入口の静的解析に基づく。実機確認は別途必要。
画面サイズの根拠は[インターフェースのガイドライン](../../docs/interface-guideline.md)。
