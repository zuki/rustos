# Lab 5: マルチコアとネットワーク

## はじめに

現在のところ、カーネルはRPiボードにある4つのコアのうち1つのコアしか利用
していません。この課題では、他の3つのコアを有効にし、既存のコンポーネントを
調整してプログラムを正しく並列実行できるようにします。その後、RPi用の既存の
イーサネット実装 (USPi) と最小限のTCPスタック (smoltcp) をカーネルに統合し、
ホストコンピュータとRaspberry Piボードをイーサネットケーブルを介して通信
できるようにします。最後にエコーサーバをカーネル上のユーザプログラムとして
書き、ホストコンピュータからnetcatコマンドを使ってそのサーバとやりとり
します。

## フェーズ 0: 始めるために

lab5 用の更新データをgitリポジトリから各自の開発マシンに取り込みます。

```bash
$ git fetch skeleton
$ git merge skeleton/lab5
```

以下はリポジトリのディレクトリ構成です。この課題で作業するディレクトリには
`*`マークが付いています。

```bash
.
├── bin : common binaries/utilities
├── doc : reference documents
├── ext : external files (e.g., resources for testing)
├── tut : tutorial/practices
│    ├── 0-rustlings
│    ├── 1-blinky
│    ├── 2-shell
│    ├── 3-fs
│    ├── 4-spawn
│    └── 5-multicore : questions for lab5 *
├── boot : bootloader
├── kern : the main os kernel *
├── lib  : required libraries
│     ├── aarch *
│     ├── kernel_api *
│     ├── fat32
│     ├── pi *
│     ├── shim
│     ├── stack-vec
│     ├── ttywrite
│     ├── volatile
│     └── xmodem
└── user : user level program *
      ├── fib
      ├── sleep
      └── socket *
```

## マージガイドライン

作業を進める前にコンフリクトを解決する必要があるかもしれません。たとえば、
次のようなメッセージが表示された場合:

```bash
Auto-merging kern/src/main.rs
CONFLICT (content): Merge conflict in kern/src/main.rs
Automatic merge failed; fix conflicts and then commit the result.
```

コンフリクトを解決するには`main.rs`ファイルを修正する必要があります。
Lab 4での変更をすべて残しておくようにしてください。すべてのコンフリクトが
解決したら`git add`と`git commit`で解決したファイルを追加してください。
マージコンフリクトを解決する方法については[githowto.comのチュートリアル](https://githowto.com/resolving_conflicts)を
参照してください。

Lab 4から様々なデザインが変更されています。マージガイドラインは以下の
変更概要を参照してください。

- Safe / Unsafe の変更
  Rustの安全保障により適合するようにいくつかの safe / unsafe 定義が変更
  されました。この点に関するマージコンフリクトがあった場合は更新された
  定義に従ってください。
- スケジューラ
    - GlobalSchedulerのその定義で`Box`を使うようになりました。これに伴い
      `Scheduler::new()`関数を更新しました。
    - タイマー操作を`start()`から`initialize_global_timer_interrupt()`に
      移動しました。この関数を`GlobalScheduler::start()`で呼び出してくだ
      さい。
- ページテーブル
  カーネルがローカルタイマーアドレスにアクセスできるように`IO_BASE_END`の
  値を上げました。これをサポートするためにカーネルページテーブルはこれまでの
  2つから3つのL3エントリを使用するようになりました。
  `kern/src/vm/pagetable.rs`にある関連する関数を調整してください。
- VMM
  VMMが以前より多くのフィールドを含むようになりました。`initialize()`で
  カーネルテーブルのベースアドレスを計算して`self.kern_pt_addr.store(kern_pt_addr, Ordering::Relaxed);`
  で`kern_pt_addr`フィールドに保存してください。この行が何を意味するかは
  このラボを通して学ぶことになります。

  さらに、`setup()`が`initialize()`で自動的に呼び出されなくなりました。
  これは、仮想メモリーマネージャを初期化する動作とカレントコアのMMUを
  セットアップする動作を分離するためであり、これにより複数のコアにおいて
  両者を独立に呼び出せるようになります。`kmain()`で`VMM.initialize()`の
  直後に`VMM.setup()`の呼び出しを追加する必要があります。
- IRQ / トラップ
  IRQは再設計されトレイトベースのロジックを使用するようになりました。
  `kern/src/traps/irq.rs`の変更箇所を読んでください。関数`register()`と
  `invoke()`を変更し、必要に応じて`handle_exception()`も変更してください。
- `write_str` syscall
  以前のラボではシリアルに1バイトを出力する`write()`システムコールしか
  ありませんでした。あらたに`write_str()` syscallを追加しました。これは
  ユーザからsliceを受け取りatomicallyに出力します。`kerenl.api`ライブラリは
  これを使用するように変更されました。ユーザプログラムを再コンパイルして
  SDカードにコピーしてください。

## ロギング基盤

カーネルコードではメッセージロギングに`kprintln!`ではなく、Rustの`log`
クレートを使うようになりました。これにより`trace!`, `debug!`, `info!`,
`warn!`, `error!`の5つのロギングマクロが使えるようになります。

ロギングコードは`kern/src/logger.rs`で定義されています。ビルド時に
`VERBOSE_BUILD`環境変数が設定された場合（たとえば、`VERBOSE_BUILD=1 make`）、
すべてのログが有効になります。そうでなければtraceレベルのログは表示され
ません。各状況においてどのレベルのログを使うべきかについては具体的な
要件はありませんが、簡単なガイドラインを以下に示します。

- Trece: カーネルデバッグには役立つがデフォルトで有効にするには冗長
  すぎる情報
    - スケジューラスイッチログ
    - IRQ割り込みログ
- Debug: 開発者が興味を持ちそうな情報
    - ページテーブルアドレス
- Info: カーネルユーザが興味を持ちそうな情報
    - システムのメモリ容量
    - カーネルの初期化ステータス
- Warn: 例外的なエラー状況の表示
    - メモリ不足
    - ユーザプログラムの不明な例外
- Error: カーネルの通常実行中には発生してはならない事象の表示
    - デバッグのアサーション違反
    - カーネル内部の不明な例外

## ARMドキュメント

lab 4で紹介した次の3つのドキュメントに加えて

- [ARMv8リファレンスマニュアル](https://tc.gts3.org/cs3210/2020/spring/r/ARMv8-Reference-Manual.pdf)
  ARMv8アーキテクチャの公式リファレンスマニュアルです。アーキテクチャ全体を
  一般的な方法で網羅する完全マニュアルです。Raspberry Pi 3向けのアーキ
  テクチャの具体的な実装についてはARM Cortex-A53マニュアルを参照して
  ください。このマニュアルのセクションは([ref](https://tc.gts3.org/cs3210/2020/spring/r/ARMv8-Reference-Manual.pdf): C5.2)と
  いう形の注記で参照し、ARMv8リファレンスマニュアルのセクションC5.2を参照
  する必要があることを示します。
- [ARM Cortex-A53マニュアル](https://tc.gts3.org/cs3210/2020/spring/r/ARM-Cortex-A53-Manual.pdf)
  Raspberry Pi 3で使用されているARMv8 (v8.0-A)アーキテクチャの具体的な
  実装に関するマニュアルです。このマニュアルのセクションは([A53](https://tc.gts3.org/cs3210/2020/spring/r/ARM-Cortex-A53-Manual.pdf): 4.3.30)と
  いう形の注記で参照し、ARM Cortex-A53マニュアルのセクション4.3.30を参照
  する必要があることを示します。
- [ARMv8-Aプログラマガイド](https://tc.gts3.org/cs3210/2020/spring/r/ARMv8-A-Programmer-Guide.pdf)
  ARMv8-Aプロセスのプログラミング方法に関する高レベルのガイドです。
  このマニュアルのセクションは([guide](https://tc.gts3.org/cs3210/2020/spring/r/ARMv8-A-Programmer-Guide.pdf): 10.1)と
  いう形の注記で参照し、ARMv8-Aプログラマガイドのセクション10.1を参照
  する必要があることを示します。

Lab 5ではさらに2つのドキュメントを使用します。

- [AArch64プログラマガイド: 汎用タイマー](https://tc.gts3.org/cs3210/2020/spring/r/aarch64-generic-timer.pdf)
  ARMアーキテクチャの汎用タイマーに関するガイドです。このマニュアルの
  セクションは([timer](https://tc.gts3.org/cs3210/2020/spring/r/aarch64-generic-timer.pdf): 3.2)と
  いう形の注記で参照し、AArch64プログラマガイド: 汎用タイマーのセクション
  3.2を参照する必要があることを示します。
- [Quad-A7 Control](https://tc.gts3.org/cs3210/2020/spring/r/QA7_rev3.4.pdf)
  Quad-A7制御に関するガイドで、コアごとのタイマーと割り込み処理に関する
  説明があります。このマニュアルのセクションは([QA7](https://tc.gts3.org/cs3210/2020/spring/r/QA7_rev3.4.pdf): 4.10)と
  いう形の注記で参照し、Quad-A7 Controlのセクション4.10を参照する必要が
  あることを示します。

これら5つの文書はすべて研究室リポジトリの`doc/`サブディレクトリにあります。
今すぐこの5つのドキュメントをダウンロードして、すぐ手の届くところに置いて
おくことを勧めます。

## フェース 1: マルチコアの有効化

### サブフェース A: 他のコアを起床させる

### サブフェース B: Mutex再び

#### Per-coreデータ管理

#### メモリ一貫性モデル

#### メモリバリアとアトミック命令

### サブフェーズ C: マルチコアスケジューリング

#### Per-core IRQ処理

#### スケジューラの修正

## フェーズ 2: TCPネットワーキング
