#!/bin/sh
# Talk to Anna, the village baker (a ve-npc character with a locked secret), through a real model.
#   engine/tools/talk-to-anna.sh                  # the Claude CLI with Haiku (fast and cheap)
#   engine/tools/talk-to-anna.sh sonnet           # another Claude model
#   MODEL_CMD='ollama run llama3.2' engine/tools/talk-to-anna.sh   # any command that reads the prompt on stdin and prints the reply
#   CHARACTER=path/to/character.json engine/tools/talk-to-anna.sh  # another character (see ve-npc's Character)
# Type a line and press Enter. Commands: ':memory' shows what she remembers, ':save file.json' saves the conversation, an empty line quits.
# Things to try: give her something, ask what you gave her later, ask what it costs, then try to get her to admit who she owes money to.
set -eu
cd "$(dirname "$0")/.."
MODEL="${1:-haiku}"
CMD="${MODEL_CMD:-claude -p --model $MODEL --tools \"\" --no-session-persistence --system-prompt \"You are the dialogue engine of a video game. Follow the reply format in the prompt exactly and output only the JSON object.\"}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
cargo build -q -p ve-npc --example chat
if [ -n "${CHARACTER:-}" ]; then
  exec "$CARGO_TARGET_DIR/debug/examples/chat" --character "$CHARACTER" --cmd "$CMD" --timeout 90
fi
exec "$CARGO_TARGET_DIR/debug/examples/chat" --cmd "$CMD" --timeout 90
