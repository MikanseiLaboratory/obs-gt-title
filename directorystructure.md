# ディレクトリ構成

```
obs-gt-title/
├── Cargo.toml                 ワークスペース
├── Cargo.lock
├── rust-toolchain.toml        Rust 1.98.0
├── technologystack.md
├── directorystructure.md
├── LICENSE                    GPL-2.0
├── README.md
├── .github/workflows/
│   ├── ci.yml                 push / pull request の fmt、clippy、test、3 OS ビルド
│   └── release.yml            vX.Y.Z タグで GitHub Release と Windows インストーラー
├── installer/
│   └── obs-gt-title.iss       Inno Setup 6
├── scripts/
│   ├── install.ps1            Windows の OBS プラグインディレクトリへコピー
│   ├── install.sh             Linux / macOS
│   ├── package.ps1            dist/ に Windows 用バイナリを集める
│   ├── package.sh             Linux / macOS
│   └── installer.ps1          Inno Setup でセットアップ exe を作る
├── fixtures/
│   ├── fonts/                 テスト用 Roboto（Apache-2.0）
│   ├── synthetic/             再配布できる合成 GTXML
│   └── golden/                時刻を固定した RGBA の PNG
└── crates/
    ├── gt-native-renderer/    OBS に依存しないシーングラフ、フォント、ラスタ、再生
    │   └── tests/title.rs
    └── obs-gt-title/          cdylib。入力ソース、プロパティ、GPU 合成
```

`gt-native-renderer` が GT IR をシーンにし、文字と図形を RGBA にする。`obs-gt-title` はその結果をテクスチャとして OBS に渡す。ファイル読込は描画スレッドの外で行う。
