# OBS で GT Title をソースとして出す

`.gtzip` と `.gtxml` を、Browser Source を使わず OBS の入力ソースとして描画するプラグインです。テキストとフォントは TextBlock ごとに OBS のプロパティから変えられ、Storyboard の再生に追従します。

初回は「ビルドして OBS に置く」まで読めば足ります。対応範囲と確認手順は、必要なときに後ろを見てください。

前提は Rust 1.98 と、リンク対象の OBS Studio 32.2 系です。Windows、macOS、Linux を同じソースでビルドします。

## リリースビルドを OBS のプラグイン置き場へコピーする

依存の取得とビルド:

```text
cargo test -p gt-native-renderer
cargo build -p obs-gt-title --release
```

`obs-sys` はビルド時に OBS のライブラリを探すので、先に OBS Studio を入れてください。Windows ではインストール先の `obs.dll` から import lib を作ります。

配置:

| OS | コピー先 |
| --- | --- |
| Windows | `%ProgramFiles%\obs-studio\obs-plugins\64bit\obs_gt_title.dll` |
| Linux | `~/.config/obs-studio/plugins/obs-gt-title/bin/64bit/libobs_gt_title.so` |
| macOS | `~/Library/Application Support/obs-studio/plugins/obs-gt-title/bin/64bit/libobs_gt_title.dylib` |

```text
powershell -File scripts/install.ps1
./scripts/install.sh
```

Windows の Program Files へのコピーは、そのディレクトリに書ける権限が要ります。OBS を再起動するとソース一覧に **GT Title** が出ます。data フォルダは不要です。合成シェーダはプラグイン内部でコンパイルします。

`scripts/package.ps1` と `scripts/package.sh` は、同じバイナリを `dist/` にまとめます。Windows で Inno Setup 6 が入っていれば、`scripts/installer.ps1` が `dist/obs-gt-title-windows/obs-gt-title-0.1.0-windows-setup.exe` を作ります。このインストーラーはレジストリの OBS の場所を見て、64bit プラグインフォルダへ DLL をコピーします。

`v1.2.3` のように、`v` とピリオド区切りの数字 3 つだけを付けたタグを push すると、GitHub Actions がテストと 3 OS のビルドを走らせ、GitHub Release に Windows インストーラーを載せます。`v1.2.3-rc1` はリリースを作りません。Linux と macOS は同じ Release に共有ライブラリを付けます。配置先は上の表と同じです。

## ソースを追加してから触る項目

1. ソースで GT Title を追加する。ファイルを読むまでは 1920×1080 の空のソースです。
2. **GTファイル** で `.gtzip` または `.gtxml` を選ぶ。読込は描画を止めないワーカーで行い、幅と高さは GT の Composition になります。
3. 読み終わると TextBlock ごとの **テキスト** と **フォント** が出ます。フォントの初期値は GT 側の指定です。family、style、size、太字、斜体、下線、打ち消し線を上書きできます。
4. テキストを変えると、その文字と依存する Bounding だけを作り直し、DataChangeOut があれば終わってから値を反映し、DataChangeIn を続けます。フォント変更はすぐ反映し、DataChange は再生しません。
5. **ストーリーボード** を変えるとその Storyboard を再生します。ソースの表示とメディアの Restart は TransitionIn、メディアの停止と **TransitionOutを再生** は TransitionOut です。一時停止は時計を止めます。表示中の Continuous は、メディア状態が再生中のままでなくても進みます。
6. **再読込** は同じファイルを読み直します。チェックは押したあと外れます。
7. 失敗した読込は直前のシーンを残し、OBS ログにファイル名、原因、gt-parser の警告を出します。指定フォントが OS に無いときは、フォント一覧から決まる代替を使い、`font.fallback` をログに残します。

設定キーはシーン内のパスから安定して作ります。表示名は GT のオブジェクト名です。シーンを保存して OBS を開き直すと、ファイル、文言、フォント、再生対象が戻ります。

## 描画で割り切っていること

vMix の GPU Title とピクセルを揃えることは目標にしていません。未対応や近似は、黙って消さずに OBS ログへ警告します。DataChange と AutoSize はこのプラグインが処理するので、gt-parser の「未対応」警告からは外しています。

- 文字、Rectangle、Ellipse、Triangle、Image、Picture Fill、線形および放射グラデーション、Size Mode、動的 Bounding を描く。
- Reveal、Fade、Fly/Move、ZoomFade、Scale、Rotate/Spin、Delay、Duration、Reverse、Direction、Linear/Cubic/Bounce を再生する。Ticker と Image Sequence も同じ時間軸です。
- 埋め込み画像の QR は Image として描く。Text3D、Cube、画像の入っていない QR、Replace や Additive 以外の合成は 2D 近似か警告です。Replace と Additive はブレンドです。
- Mask はマスク対象の静止位置からアルファを焼き込みます。Reflection は薄くした反転コピー、Shadow は半径 24 までのぼかしです。Skew は GT の指定どおりには変形しません。

## 手元で確認すること

自動テストは `cargo test -p gt-native-renderer` です。シーングラフ、文字の差し替え、フォントの上書きと代替、Bounding、補間、固定フォントでの PNG を見ます。ゴールデン用フォントは `fixtures/fonts/Roboto-Regular.ttf`（Apache-2.0）です。フィクスチャは再配布できる合成 GTXML だけです。

OBS 上の操作は、各 OS で次を目で見てください。このリポジトリの自動テストは OBS の画面までは駆動しません。

- 透過と、GetWidth / GetHeight が Composition の寸法であること
- 再読込、テキスト変更、フォント変更
- TransitionIn、DataChange、Continuous、TransitionOut
- シーン保存後にファイル、文言、フォント、ストーリーボードが戻ること
- 壊れたファイルを読み込んでも、直前の絵が残りログに理由が出ること

## ライセンス

プラグインは GPL-2.0-only です。全文は [LICENSE](LICENSE) にあります。テスト用 Roboto は [fixtures/fonts/LICENSE](fixtures/fonts/LICENSE) の Apache-2.0 です。
