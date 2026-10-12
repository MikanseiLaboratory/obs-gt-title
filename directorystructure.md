# ディレクトリ構成

```
obs-gt-title/
├── Cargo.toml                 ワークスペース設定
├── Cargo.lock                 依存関係ロックファイル
├── rust-toolchain.toml        Rust ツールチェーン固定 (1.98.0)
├── technologystack.md         技術スタックおよび依存ライブラリ一覧
├── directorystructure.md      リポジトリ構造解説
├── LICENSE                    GPL-2.0 ライセンス全文
├── README.md                  プロジェクト説明書および利用ガイド
├── .github/workflows/
│   ├── ci.yml                 CI ワークフロー（fmt, clippy, test, 3 OS ビルド）
│   └── release.yml            リリースワークフロー（vX.Y.Z タグ時の 3 OS ビルドとインストーラー作成）
├── installer/
│   └── obs-gt-title.iss       Inno Setup 6 インストーラー定義
├── scripts/
│   ├── install.ps1            Windows 向け OBS プラグイン配置スクリプト
│   ├── install.sh             Linux / macOS 向け OBS プラグイン配置スクリプト
│   ├── package.ps1            Windows 向け配布パッケージ作成スクリプト
│   ├── package.sh             Linux / macOS 向け配布パッケージ作成スクリプト
│   └── installer.ps1          Windows 用 Inno Setup インストーラー作成スクリプト
├── fixtures/
│   ├── fonts/                 テスト用フォント (Roboto, Apache-2.0)
│   ├── synthetic/             合成テスト用 GTXML フィクスチャ
│   └── golden/                レンダリング検証用ゴールデン画像 (PNG)
└── crates/
    ├── gt-native-renderer/    OBS 非依存の描画コア（シーングラフ、フォント解決、CPU ラスタライズ、再生制御）
    │   └── tests/title.rs     統合テスト群
    └── obs-gt-title/          OBS Studio プラグイン本体（cdylib: ソース登録、プロパティ UI、GPU テクスチャ合成）
```

### アーキテクチャの責務分離
- **`gt-native-renderer`**: GT の中間表現（IR）からシーングラフを構築し、文字や図形を CPU 上で RGBA ラスタライズします。OBS 固有の API に一切依存しないため、単体テストやゴールデン画像検証が容易です。
- **`obs-gt-title`**: OBS Studio の入力ソースとして登録され、ユーザープロパティ UI の提供、ワーカースレッドによる非同期ファイル読み込み、ラスタライズ結果の GPU テクスチャ合成を行います。
