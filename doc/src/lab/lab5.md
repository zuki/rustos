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

ラボ4ではプリエンプティブスケジューリングについて学びました。これは
コンテキストスイッチングにより1つのカーネル内で複数のユーザプログラムを
同時に実行することを可能にするものでした。複数のプログラムがカーネル上で
_同時 (concurrently)_ 実行されていましたが、ある時点では1つのユーザ
プログラムだけがコアを占有していたことに注意してください。このフェーズでは
RPiボードにある残りの3つのコアを有効にしてユーザプログラムの _並列 (parallel)_
実行をサポートします。並列プログラミングにはシングルスレッドプログラミングには
存在しない多くの固有の問題があります。これを解決するためにミューテックス、
IRQハンドラ、スケジューラの設計を再検討し、マルチコア環境に合わせて調整します。

### サブフェース A: 他のコアを起床させる

このサブフェーズではスピンテーブル機構を使用してBCM2837の残りの3つのコアを
有効にします。

#### スピンテーブル

CPUのすべてのコアはメインメモリ（RAM）を共有しています。そのため、RAMを
コア間の通信媒体として利用できます。_スピンテーブル_ はこのRAMの特性を
利用した起動メカニズムであす。RPiの電源を入れると最初のコアであるコア0は
`kern/src/init.rs`で定義されている`_start`関数にジャンプします。他のコアは
すべてカーネルの外でスピンして、スピニングアドレスをポーリングしています。

[RPiファームウェア](https://github.com/raspberrypi/tools/blob/b0c869bc929587a7e1d20a98e2dc828a24ca396a/armstubs/armstub8.S#L132-L154)の
以下のコードがスピンテーブル機構を実装しています。

```
in_el2:
    mrs x6, MPIDR_EL1
    and x6, x6, #0x3
    cbz x6, primary_cpu

    adr x5, spin_cpu0

secondary_spin:
    wfe
    ldr x4, [x5, x6, lsl #3]
    cbz x4, secondary_spin
    mov x0, #0
    b boot_kernel

primary_cpu:
    ldr w4, kernel_entry32
    ldr w0, dtb_ptr32

boot_kernel:
    mov x1, #0
    mov x2, #0
    mov x3, #0
    br x4
```

RPiが起動するとコア0はシンボル`kernel_entry32`からカーネルアドレスを
ロードしてそのアドレスに分岐します。RPiファームウェアはこのルーチンの前に
`config.txt`で指定された`kernel_address`の値を`kernel_entry32`にセットします。
他のすべてのコアは`secondary_spin`ループ内で`wfe`でスピンします。ループ中、
これらのコアは`spin_cpu0 + 8 * core_idx`から8バイトのアドレスをロードし、
0でなければそのアドレスに分岐します。そのため、他のコアを起床させるに
コア0は開始アドレスをスピニングアドレスに書き込み、`sev`命令でイベントを
送信する必要があります。他のコアのエントリポイントには`init::start2`を
使います。

各コアは他のコアと干渉しないように各自のスタックポインタを使用する必要が
あります。関数`_start`はコア0のスタックレジスタとして`KERN_STACK_BASE`を
割り当てています。私たちのカーネル設計ではコア`i+1`はコア`i`のスタックの
直下を使用します。すなわち、コア`i`のスタックポインタは
`KERN_STACK_BASE - KERN_STACK_SIZE * i`です。

#### 実装

これでコアの初期化ルーチンを実装する準備ができました。`kern/src/main.rs`に
ある`kmain()`と`kern/src/init.rs`にある`initialize_app_cores()`,
`start2()`, `kmain2()`を作成します。

これらの関数は好きな順番で実装死て構いません。

- **`initialize_app_cores()`では`start2()`のアドレスを各コアのスピニングアドレスに書き込みます**

    スピニングベースである`spin_cpu0`は`pi/src/common.rs`において定数
    `SPINNING_BASE`として定義されています。コア0は各コアのスピニング
    アドレスを計算してそのアドレスに`start2()`のアドレスを書き込み、
    `sev()`を実行し、各コアからの確認を待ちます。他のコアは完全に目覚めたら、
    `kmain2()`でスピニングアドレスに0を書き込みます。コア0はすべてのコアの
    スピニングアドレスが0で上書きされたことを確認した上で
    `initialize_app_cores()`から復帰する必要があります。

- **`start2()`ではスタックポインタをセットして`kinit2()`に分岐します**

    上で説明したようにスタックポインタをセットしてください。コアの
    インデックスは`MPIDR_EL1`から抽出できることを思い出してください。
    次に、スタックレジスタをセットしてから`kinit2`に分岐してください。
    スタックポインタを変更しているので`start2()`ではスタック変数を使用
    しないように注意してください。

    **警告: `aarch64::affinity()`は動作しない可能性があります**

    `aarch64::affinity()`はインライン化されることが保証されておらず、
    関数を呼び出すためにスタック空間を使用する可能性があります。
    スタックポインタはまだセットしていないので`aarch64::affinity()`を
    使うことができないかもしれません。`MPIDR_EL1`レジスタに直接
    アクセスしてみてください。

- **`kmain2()`ではメッセージを表示し、コアが使用可能であることを確認します**

    初期化が成功すると各コアは`kmain2()`に到達し、**EL1**で実行を開始します。
    `kmain2()`ではコアのスピンingアドレスンに0を書き込んでコア0に通知します。
    その後、コンソールに任意のメッセージを表示して無限ループに入ってください。
    新しく追加されたロギングマクロを使ってください。

コードを書き終えたら、`kmain()`を次のように変更してください。

```rust
unsafe fn kmain() -> ! {
    ALLOCATOR.initilaiize();
    FILESYSTEM.initialize();
    VMM.initialize();
    SCHEDULER.initialize();

    init::initialize_app_cores();
    VMM.setup();

    SCHEDULER.start();
}
```

RPiボード上でカーネルを実行すると各コアからの初期化メッセージが表示される
はずです。ただし、これらの新しいコアはまだ何の役にも立っていません。私たちの
カーネルのいくつかのコンポーネントはまだパラレル対応になっていないからです。
次のステップはこれらの古い設計を修正することです。

**警告: 不健全なミューテックスに注意**
> 私たちのミューテックスは健全ではないので、内部的にミューテックスを使用
> している`kprintln!()`やロギングマクロを複数のコアからアクセスすると
> デッドロックを起こしたり、データ競合を引き起こしたりする可能性があります。
> これはまれなケースであり、十分な時間リトライすれば成功するケースが
> 見られるはずです。すぐに修正する予定です。

**質問 (spin-address): アドレスをハードコードせずにスピンベースを検出する方法は(スピンアドレス)**
> Raspberry Piのスピンアドレスをカーネルにハードコードしているので
> スピンアドレスの異なる他のハードウェアではカーネルは動作しません。
> Linuxカーネルはこの問題をどのように解決していますか。

### サブフェース B: Mutex再び

#### Per-coreデータ管理

#### メモリ一貫性モデル

#### メモリバリアとアトミック命令

### サブフェーズ C: マルチコアスケジューリング

#### Per-core IRQ処理

#### スケジューラの修正

## フェーズ 2: TCPネットワーキング
