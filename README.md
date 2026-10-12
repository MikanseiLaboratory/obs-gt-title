# obs-gt-title

vMix GT Title (`.gtzip` / `.gtxml`) を OBS Studio で直接描画するネイティブプラグインです。ブラウザソースを介さず、OBS 上でタイトルのアニメーション再生やテキスト・フォントの編集を行えます。

Windows、macOS、Linux に対応しています。

## 主な特徴

- **ネイティブ描画**: ブラウザソースを経由せず、OBS のパイプライン上で直接合成・描画
- **プロパティ編集**: OBS のソース設定から TextBlock ごとに文言やフォントを直接変更可能
- **アニメーション再生**: `TransitionIn` / `TransitionOut`、`DataChange`、`Continuous` などの Storyboard に対応
- **マルチプラットフォーム**: Windows（インストーラー提供）、macOS、Linux をサポート

## 入手方法

最新のインストーラーおよび各 OS 向けバイナリは [GitHub Releases](https://github.com/MikanseiLaboratory/obs-gt-title/releases) よりダウンロードできます。

- **Windows**: `obs-gt-title-<version>-windows-setup.exe`
- **Linux**: `libobs_gt_title.so`
- **macOS**: `libobs_gt_title.dylib`

インストール後、OBS Studio を再起動し、ソース追加から **GT Title** を選択してファイル（`.gtzip` または `.gtxml`）を指定してください。

## ライセンス

[GPL-2.0-only](LICENSE)
