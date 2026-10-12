# 技術スタック

バージョンはここを正とする。上げるときは理由を示して承認を得てから変える。Cargo.toml の範囲指定と、解決結果の Cargo.lock は分けて書く。

## 言語とビルド

| 項目 | 版 |
| --- | --- |
| Rust | 1.98.0（`rust-toolchain.toml`） |
| edition | 2024 |
| パッケージ版 | 0.1.0 |
| ライセンス | GPL-2.0-only |

プラグインは `obs-wrapper` にリンクするため GPL-2.0-only とする。ライブラリ名は `obs_gt_title`（ハイフンは cdylib 名に使えない）。

## 依存

| クレート | マニフェスト | lock で解決した版 | 用途 |
| --- | --- | --- | --- |
| gt-core | git `a8ace88680f628a8f2f38d428dabc14f4607a438` | 同 commit | GT の Package / GtDocument / Storyboard。XML 解析は再実装しない |
| obs-wrapper | git `920faa6da3224485f3891f6655bd9e1ba437f1f7` 0.4.1 | 同 commit | OBS 32.2 系 ABI。`obs-sys` のサブモジュールに obs-studio のヘッダがある |
| cosmic-text | 0.19.0 | 0.19.0 | 文字組み。ヒントは無効（OS 間でゴールデンを揃える） |
| tiny-skia | 0.12.0 | 0.12.0 | オブジェクト単位の CPU ラスタ |
| image | 0.25.10（png / jpeg / gif / bmp / webp） | 0.25.10 | 埋め込み画像と GIF |
| tokio | 1.53.1（fs, io-util, macros, rt, sync） | 1.53.2 | GT パッケージの非同期読込。プラグイン側はワーカーから `load_path_blocking` を呼ぶ |
| zip | 8.6.0 | 8.6.0 | gtzip。gt-parser と同じ系列に固定 |
| thiserror | 2.0.21 | 2.0.21 | レンダラーのエラー型 |
| log | 0.4.28 | 0.4.34 | OBS ログへ転送 |

HTML レンダラーと Browser Source は描画経路に使わない。vMix GPU Title とのピクセル完全一致は目標にしない。

## OBS

- 入力ソース ID は `gt_title`、表示名は `GT Title`。
- 描画は CPU でオブジェクトごとのプリマルチプライド RGBA を作り、OBS のグラフィクススレッドでテクスチャへ載せて合成する。
- シェーダはプラグイン内の文字列から `gs_effect_create` する。data ディレクトリは置かない。
- ビルドマシンに OBS Studio が入っている必要がある。Windows では import lib をインストール済みの `obs.dll` から生成する。

## 配布

- Windows のインストーラーは Inno Setup 6。スクリプトは `installer/obs-gt-title.iss`。
- `vX.Y.Z`（例: `v0.1.0`）のタグ push で `.github/workflows/release.yml` が動き、GitHub Release にインストーラーを載せる。プレリリース付きのタグは対象外。
