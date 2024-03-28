# lab 0: Rustlings

## はじめに

[省略]

## 概要

プログラミング言語のマスターには時間がかかります。実際、ひとつのプログラミング
言語を本当にマスターするには長い時間、おそらく一生、かかります。言い換えれば、
1週間でRustの真のマスターになるというのは非現実的です。しかし、今学期の終わり
にはRustで何かをいじり始められることを願っています。

ウォームアップとして、Rustプログラミング言語の構文と意味の基礎を理解する
ために用意した一連の質問に答えていきます。

環境をセットアップするために、まず必要な依存システムとRustのエコシステム、
`rustc`と`cargo`をインストールしましょう。ここでは最新のLTS Ubuntuディストリ
ビューション（Ubuntu 18.04 LTS）で課題を行うことを想定しています。Ubuntuを
ネイティブで実行していない場合は、ツールページを確認してUbuntu仮想マシンを
準備してください。また、`git`に慣れていない人はまず[Pro Git Book](https://git-scm.com/book/en/v2)を読んでください。

```bash
$ git clone https://github.com/sslab-gatech/cs3210-rustos-public.git --origin skeleton rustos
$ cd rustos

# 次のコマンドは自動的に環境を設定します
$ bin/setup.sh
...

$ export PATH=$HOME/.cargo/bin:$PATH
$ rustc --version
rustc 1.37.0-nightly (0af8e872e 2019-06-30)
```

レポジトリは追加ダウンロードや依存ファイルが必要ないようにすべてを置くように
努めています。現在は次のようになっています。

```bash
.
├── bin : common binaries/utilities (e.g., objdump for aarch64)
├── doc : reference documents
└── tut : tutorial/practices (i.e., no further reuse for later labs)
    └── 0-rustlings : this contains files for lab0
```

## Rustlings

Lab 0の目的は[Rust By Example](https://doc.rust-lang.org/rust-by-example/index.html)の
第1章から第18章までを読むように導くことです。

理解度を確認するためにRustlingsを使用します。Rustlingsは簡単な練習問題を解く
ことでRustに慣れることができる良い方法です。コンパイラのエラーメッセージを
理解し、それを解決する一連のインタラクティブなトレーニングを提供します。練習
問題を解く過程で[The Rust Book](https://doc.rust-lang.org/book/index.html)の
様々な章を読無事になるでしょう。また、コースのリファレンスページではRustに
関するより多くのヒントを見つけることができます。

```bash
$ cd tut/0-rustlings
$ ./rustlings help
...
SUBCOMMANDS:
     help      Prints this message or the help of the given subcommand(s)
     hint      Returns a hint for the current exercise
     run       Runs/Tests a single exercise
     verify    Verifies all exercises according to the recommended order
     watch     Reruns `verify` when files were edited
```

練習を開始するには`watch`コマンドを実行します。どのエクササイズに取り組むかを
適切な順番でガイドしてくれます。

```bash
$ ./rustlings watch
```

各練習問題（`exercises/`にあるファイル）を次のように実行することもできます。

```bash
./rustling run [name].

$ ./rustlings run variables1
 ! Compilation of exercises/variables/variables1.rs failed! Compiler error message:

 error[E0425]: cannot find value `x` in this scope
   --> exercises/variables/variables1.rs:12:5
    |
 12 |     x = 5;
    |     ^ not found in this scope

 error[E0425]: cannot find value `x` in this scope
   --> exercises/variables/variables1.rs:13:36
    |
 13 |     println!("x has the value {}", x);
    |                                    ^ not found in this scope

 error: aborting due to 2 previous errors

 For more information about this error, try `rustc --explain E0425`.
```

練習問題が難しすぎると感じたら次のようにrustlingsに問題のヒントを求める
と良いでしょう。

```bash
$ rustlings hint variables1
Hint: The declaration on line 12 is missing a keyword that is needed in Rust
to create a new variable binding
```

この課題を完了するためには`.rusting verify`を実行して準備されたすべての
練習問題が成功裏に完了したことを確認してください！

[以下略]
