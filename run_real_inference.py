#!/usr/bin/env python3
"""Run real inference using llama-cpp-python"""

import sys
import os

try:
    from llama_cpp import Llama
    
    model_path = "models_cache/mistral-7b-instruct-v0.2.Q4_K_M.gguf"
    
    if not os.path.exists(model_path):
        print(f"Error: Model file not found at {model_path}", file=sys.stderr)
        sys.exit(1)
    
    print(f"Loading model: {model_path}", file=sys.stderr)
    print("This may take a moment...", file=sys.stderr)
    
    # Initialize Llama
    llm = Llama(
        model_path=model_path,
        n_ctx=2048,      # Context window
        n_threads=8,    # Number of CPU threads
        verbose=False   # Suppress verbose output
    )
    
    print("Model loaded. Generating response...", file=sys.stderr)
    print("", file=sys.stderr)
    
    # Generate response
    prompt = "Describe a cat."
    
    response = llm(
        prompt=prompt,
        max_tokens=256,
        temperature=0.7,
        top_p=0.9,
        echo=False,  # Don't echo the prompt
        stop=["\n\n\n"]  # Stop on triple newline
    )
    
    # Extract and print the generated text
    generated_text = response['choices'][0]['text'].strip()
    
    print("=" * 70)
    print("REAL AI RESPONSE:")
    print("=" * 70)
    print(generated_text)
    print("=" * 70)
    
except ImportError:
    print("Error: llama-cpp-python not installed", file=sys.stderr)
    print("Install it with: pip install llama-cpp-python", file=sys.stderr)
    sys.exit(1)
except Exception as e:
    print(f"Error: {e}", file=sys.stderr)
    import traceback
    traceback.print_exc()
    sys.exit(1)




