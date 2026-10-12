# obs-gt-title

vMix GT Title (`.gtzip` / `.gtxml`) を、ブラウザソースを経由せず OBS Studio 上で直接レンダリングするネイティブプラグインです。
各 TextBlock のテキストやフォントを OBS のプロパティから直接編集でき、Storyboard によるアニメーション再生に対応しています。

Windows、macOS、Linux の各プラットフォームに対応しています。

---

## インストール手順

### リリースバイナリを利用する場合
[Releases](https://github.com/MikanseiLaboratory/obs-gt-title/releases) より、ご利用の OS に合わせた最新版をダウンロードしてください。

- **Windows**: `obs-gt-title-<version>-windows-setup.exe` を実行してインストールします（OBS Studio の 64bit プラグインフォルダへ自動配置されます）。
- **Linux**: `libobs_gt_title.so` を `~/.config/obs-studio/plugins/obs-gt-title/bin/64bit/` に配置します。
- **macOS**: `libobs_gt_title.dylib` を `~/Library/Application Support/obs-studio/plugins/obs-gt-title/bin/64bit/` に配置します。

配置後、OBS Studio を再起動してください。ソースの追加メニューに **GT Title** が表示されます。

### ソースコードからビルドする場合
Rust 1.98 以降および OBS Studio 32.2 系の開発環境が必要です。

```bash
# レンダラーのテスト
cargo test -p gt-native-renderer

# プラグインのリリースビルド
cargo build -p obs-gt-title --release
```

ビルド後、スクリプトで所定のプラグインディレクトリへインストールまたはパッケージングできます。

- Windows: `powershell -File scripts/install.ps1`
- Linux / macOS: `./scripts/install.sh`

`scripts/package.ps1`（Windows）または `scripts/package.sh`（Linux / macOS）を実行すると、`dist/` ディレクトリに成果物がまとめられます。Inno Setup 6 がインストールされた Windows 環境では、`scripts/installer.ps1` によりセットアップインストーラー（exe）を作成できます。

---

## 使い方とプロパティ設定

1. **ソースの追加**:
   OBS の「ソース」パネルで「+」をクリックし、**GT Title** を選択して追加します。初期状態では 1920×1080 の透明なソースが作成されます。
2. **ファイルの指定**:
   プロパティ画面の **GT Title ファイル** で、読み込む `.gtzip` または `.gtxml` を指定します。読み込みは非同期で行われるため、描画スレッドをブロックしません。読み込み完了後、ソース解像度は GT の Composition サイズへ自動設定されます。
3. **テキストとフォントの編集**:
   GT 内に含まれる TextBlock ごとに、テキスト入力欄とフォント選択欄が表示されます。
   - 初期値には GT 内で指定されたフォントやスタイルが反映されます。
   - フォントファミリ、スタイル、サイズ、太字、斜体、下線、取り消し線を個別にカスタマイズできます。
4. **動的更新と DataChange アニメーション**:
   テキストを変更すると、対象の文字テクスチャおよび依存する Bounding（枠線のサイズ自動追従など）のみが再計算されます。
   - GT 内に `DataChangeOut` アニメーションが定義されている場合、まず退出アニメーションが再生され、完了した瞬間に新しい文言が反映されて `DataChangeIn` アニメーションが再生されます。
   - フォントの変更は即座に反映されます。
5. **ストーリーボードの再生制御**:
   - **ストーリーボード**: ドロップダウンから再生したい Storyboard を選択します。
   - **ソース表示時 / Restart**: `TransitionIn` アニメーションを再生します。
   - **メディア停止 / TransitionOut を再生**: `TransitionOut` アニメーションを再生します。
   - **Continuous アニメーション**: ソースが表示されている間、継続してループ再生されます。
6. **再読み込み**:
   外部エディタで GT ファイルを更新した場合、**再読み込み** にチェックを入れることで即座に反映されます（実行後、チェックは自動的にオフに戻ります）。
7. **エラーハンドリング**:
   壊れたファイルや不正なパスを指定した場合でも直前の正常な描画状態を保持し、OBS ログにエラー詳細と警告を出力します。システムに存在しないフォントが指定された場合は、代替フォントで描画しつつログに警告を記録します。

---

## 対応機能と仕様上の制約

vMix GPU Title とのピクセル単位の完全一致は本プロジェクトの目標外とし、未対応機能や近似処理を行う箇所は OBS ログに警告を出力します。

### 対応機能
- **図形・描画**: TextBlock（各種フォント属性、AutoSize Grow / Shrink、整列）、Rectangle、Ellipse、Triangle、Image、Picture Fill、線形グラデーション（Linear）、放射グラデーション（Radial）、Size Mode（Contain / Normal / Stretch / Centered）、動的 Bounding
- **アニメーション**: Reveal、Fade、Fly / Move、ZoomFade、Scale、Rotate / Spin、Ticker（スクロール文字）、Image Sequence
- **パラメータ**: Delay、Duration、Reverse、Direction、補間イージング（Linear / Cubic / Bounce）

### 主な制約・近似処理
- **3D オブジェクト**: Text3D や Cube は 2D 近似または警告対象となります。
- **QR コード**: 埋め込み画像付きの QR コードは Image として描画されます。未埋め込みの動的生成 QR は警告が出力されます。
- **Mask / Shadow**: マスクのアルファ計算は静止時の境界ボックスを基準としています。シャドウのぼかし半径は最大 24px です。
- **合成モード**: Replace および Additive は OBS ブレンド機能として近似されます。

---

## 開発とテスト

本プロジェクトは OBS 非依存の描画コア（`gt-native-renderer`）と OBS プラグイン（`obs-gt-title`）に分離して設計されています。

```bash
# 全テストの実行（シーングラフ、フォント解決、Bounding、アニメーション補間、ゴールデン画像比較）
cargo test -p gt-native-renderer

# コードフォーマットおよび静的解析
cargo fmt --all -- --check
cargo clippy -p gt-native-renderer --all-targets -- -D warnings
cargo clippy -p obs-gt-title --no-deps -- -D warnings
```

---

## ライセンス

本プラグインは **GPL-2.0-only** の下で公開されています。ライセンスの全文は [LICENSE](LICENSE) をご参照ください。
テスト用フィクスチャに含まれる Roboto フォントは [fixtures/fonts/LICENSE](fixtures/fonts/LICENSE)（Apache License 2.0）に基づきます。
