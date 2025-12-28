#!/bin/bash
cd /mnt/c/Users/dan/punch-simple
echo "Describe a cat." | ./llama.cpp/build/bin/llama-cli -m models_cache/shards/shard-0.gguf -n 200 --temp 0.7 --top-p 0.9 -t 8 2>&1





