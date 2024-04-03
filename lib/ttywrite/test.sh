#! /bin/bash

function cleanup_and_exit() {
  kill $!
  exit $1
}

# 1バイトから512バイトの間のランダムbase64符号化文字列を生成する
function rand_string() {
  base64 < /dev/urandom | head -c $((1 + RANDOM % 512))
}

# 端末に出力する場合はカラーを使う
if [ -t 1 ]; then
  KNRM="\x1B[0m"; KRED="\x1B[31m"; KGRN="\x1B[32m"; KBLU="\x1B[34m"
else
  KNRM=""; KRED=""; KGRN=""; KBLU=""
fi

# socat(Multipurpose relay (SOcket CAT))コマンドの存在を確認
if ! command -v socat > /dev/null 2>&1; then
  echo >&2 "error: the 'socat' command is required but not installed"
  echo >&2 "help: install the 'socat' package using your package manager"
  exit 1
fi

# ttywriteプロジェクトをビルド
echo -e "${KBLU}Compiling project with 'cargo build'...${KNRM}"
if ! cargo build; then
  echo -e "${KRED}ERROR: ttywrite compilation failed${KNRM}" >&2
fi

# ptyを開く
echo -e "${KBLU}Opening PTYs...${KNRM}"
# PARAMS="pty,echo=0,raw,ispeed=19200,ospeed=19200,parenb=0,cs8,cstopb=0"
PARAMS="pty,echo=0,raw,parenb=0,cs8,cstopb=0"
socat -u ${PARAMS},link=input ${PARAMS},link=output &
sleep 1

# 端末設定
if [[ "$(uname)" = "Darwin" ]]; then
  stty -f input min 0 time 1
  stty -f output min 0 time 1
else
  stty -F input min 0 time 1
  stty -F output min 0 time 1
fi

# 入力を変えて10回テスト
for i in {1..10}; do
  echo -e "${KBLU}Running test ${i}/10.${KNRM}"

  input=$(rand_string)
  ./target/debug/ttywrite -i <(echo "${input}") -r input
  output=$(cat output)
  if [[ "${output}" != "${input}" ]]; then
    echo -e "${KRED}ERROR: input and output differ${KNRM}" >&2
    echo "${input} != ${output}" >&2
    cleanup_and_exit 1
  fi
done

echo -e "${KGRN}SUCCESS${KNRM}"
cleanup_and_exit 0
