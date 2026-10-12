# 技術スタック

本プロジェクトで採用している技術仕様および依存関係の一覧です。記載されたバージョンは本プロジェクトの基準とし、バージョン更新時は検証と承認を経るものとします。

## 開発言語とランタイム環境

| 項目 | 設定値 / バージョン |
| --- | --- |
| 言語 | Rust 1.98.0（`rust-toolchain.toml`） |
| Edition | 2024 |
| パッケージバージョン | 0.1.0 |
| ライセンス | GPL-2.0-only |

OBS プラグインは GPL ライセンスの `obs-wrapper` とリンクするため、ライセンスは GPL-2.0-only としています。また、`cdylib` の命名規則に従い、ライブラリ名は `obs_gt_title`（アンダースコア表記）としています。

## 依存ライブラリ (Crates)

| クレート名 | Cargo.toml 指定 | 解決済みバージョン (Cargo.lock) | 用途・選定理由 |
| --- | --- | --- | --- |
| gt-core | git rev `a8ace88680f628a8f2f38d428dabc14f4607a438` | 同一 commit | GT の Package / GtDocument / Storyboard 中間表現（IR）。XML 解析の重複実装を排除 |
| obs-wrapper | git rev `920faa6da3224485f3891f6655bd9e1ba437f1f7` (0.4.1) | 同一 commit | OBS Studio 32.2 系 ABI バインディング。内部で `obs-sys` を通じて libobs をリンク |
| cosmic-text | 0.19.0 | 0.19.0 | 3 OS 共通のフォント解決・レイアウト。レンダリング安定性のためヒンティングは無効化 |
| tiny-skia | 0.12.0 | 0.12.0 | オブジェクト単位の 2D ベクター / ラスター描画（CPU） |
| image | 0.25.10 (png, jpeg, gif, bmp, webp) | 0.25.10 | 埋め込み画像およびアニメーション GIF のデコード |
| tokio | 1.53.1 (fs, io-util, macros, rt, sync) | 1.53.2 | GT パッケージの非同期読み込み |
| zip | 8.6.0 | 8.6.0 | `.gtzip` 書庫の解凍 |
| thiserror | 2.0.21 | 2.0.21 | レンダラーのエラーハンドリング |
| log | 0.4.28 | 0.4.34 | OBS ログシステムへのログ転送 |

※ ブラウザソース（HTML レンダラー）は使用せず、OBS ネイティブのグラフィクスパイプラインで描画します。なお、vMix GPU Title との完全一致は非目標とし、未対応機能や近似処理は OBS ログに警告を出力します。

## OBS 統合仕様

- **入力ソース ID**: `gt_title`（UI 表示名: `GT Title`）
- **レンダリング構成**: CPU でオブジェクト単位のプリマルチプライド RGBA テクスチャをラスタライズし、OBS のグラフィクススレッド上で GPU 合成
- **シェーダ管理**: 内部組み込みシェーダから `gs_effect_create` により動的コンパイル（外部 data ディレクトリへの依存なし）
- **ビルド要件**: OBS Studio 本体のインストール（Windows 環境ではインストール済み `obs.dll` からインポートライブラリを自動生成）

## パッケージングと配布

- **Windows インストーラー**: Inno Setup 6（定義スクリプト: `installer/obs-gt-title.iss`）
- **リリース自動化**: `vX.Y.Z` 形式のタグプッシュ（例: `v0.1.0`）をトリガーとして GitHub Actions（`.github/workflows/release.yml`）が起動し、3 OS 向けバイナリおよび Windows セットアップインストーラーをビルドして GitHub Releases に公開
