# brain-usb-probe

V690_H1用の実験的なUSB通信確認アプリ。
固定文字列 `REFLEX USB PROBE v1\n` を1回だけkernelの送信バッファへ格納する。
メモリダンプや任意のファイル転送は行わない。USBの解析は [docs/usb.md](../../docs/usb.md)。

## ビルドと配置

```sh
cargo run -p reflex-cli -- build usb-probe
# Brainの通常の保存領域をマウントした場所を指定
cargo run -p reflex-cli -- deploy usb-probe /mnt/brain
sync
sudo umount /mnt/brain
```

本体でPC接続を終了し、追加アプリ `UsbProbe` を起動する。
**マウント解除してからEnterを押す。** USBの切断・DRMモードでの再接続が発生する。
WSLではUSBを再アタッチする必要がある場合がある。

- Enter: 接続を切り替えて固定文字列を送信する。
- `Hello queued`: kernelへの格納完了。PC側の受信完了ではない。
- Esc: 通信を無効化し、USB切断を待ってアプリを終了する。
- タイムアウト・エラー: Escで切断を試す。送信をやり直すにはアプリを再起動する。

終了時に以前の公開ドライブを復元する機能はない。通常の転送へ戻すときは純正PCConnectを起動する。

## Ubuntu / WSLで受信

リポジトリのルートで実行する。Ruby標準ライブラリとLinuxのSG_IOを使い、追加gemは不要。
`/dev/sgN` は再接続で変わるため、対応するデバイスを確認する。

```sh
lsusb -d 04dd:92e1
ls -l /sys/class/scsi_generic/*/device/block
sudo ruby scripts/usb-probe.rb probe /dev/sg4
sudo ruby scripts/usb-probe.rb receive /dev/sg4
```

`probe` はUSB属性とSCSI READ CAPACITYを読むだけ。
`receive` はVID/PID、revision `0103`、4096 × 512-byteの容量、未マウント状態を確認する。
条件が違えば通信パケットを書かずに終了する。通常の201 MiBディスクでは実行できない。

`receive` は状態照会→データ受信→再送要求→同一パケットの確認を行う。
成功時は固定文字列、件数 `hello_copies`、連番、`"retransmission": "identical"` をJSONで出力する。
未送信データがない場合は保持済みパケットを再送要求で取得し、`retained: true` として報告する。
固定文字列が複数件連結されていても内容を照合するが、単発送信の成功とは区別する。
ブロックキャッシュを通さず、SCSI READ(10)/WRITE(10)で各1セクターを転送する。
再実行時は保持済みパケットが返る場合がある。新しい送信を確かめる場合は本体側アプリを再起動する。

## 検証状況

ARM向けビルドとClippy、PC側Rubyの構文、SG_IO構造体、解析から生成した
制御パケットとの一致、チェックサム破損の拒否を確認。
実機の通常モード（04dd:92e1、revision 0101）で `probe` のSCSI容量照会に成功し、
411648 × 512 bytesを取得した。UsbProbeの配置・コピー照合・アンマウントも完了。
本体で `Hello queued. Read on PC.` を確認し、WindowsのGet-Diskで2097152 bytesの
公開ディスクを確認した。DRMへの切り替えと送信バッファへの格納まで進んだ。
再列挙後のWSLアタッチにはWindows管理者による `bind --force` が必要だった。
その後revision 0103、4096 × 512 bytesを照会でき、固定文字列2件が連結された40 bytesを受信。
チェックサムと連番0を確認し、再送要求後の512 bytesも完全一致した。
Escで本体アプリが終了したとの報告を受け、LinuxのUSB disconnectログと
Windows・Linux両方の接続一覧からBrainが消えたことを確認した。
2件になった原因と初回読み出しを行った主体は未確認。
このツールは最初の疎通確認用で、通信失敗後の自動復旧や大容量転送は実装していない。
