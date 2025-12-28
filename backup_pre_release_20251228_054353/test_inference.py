#!/usr/bin/env python3
"""Test inference request via WebSocket"""
import asyncio
import websockets
import json
import sys

async def test_inference():
    uri = "ws://localhost:8081"
    
    print("\n=== TESTING INFERENCE REQUEST ===")
    print("Question: what does a cow say")
    print()
    
    try:
        print("[1/4] Connecting to WebSocket server...")
        async with websockets.connect(uri) as websocket:
            print("  ✓ Connected to WebSocket")
            
            # Create query request
            query_request = {
                "query": "what does a cow say",
                "request_id": "cow-query-12345"
            }
            
            print("\n[2/4] Sending inference request...")
            print(f"  Request: {json.dumps(query_request)}")
            
            await websocket.send(json.dumps(query_request))
            print("  ✓ Request sent")
            
            print("\n[3/4] Waiting for response...")
            
            # Wait for response with timeout
            try:
                response_text = await asyncio.wait_for(websocket.recv(), timeout=120.0)
                
                print("\n[4/4] Response received!")
                print("\n" + "=" * 70)
                print("AI RESPONSE:")
                print("=" * 70)
                
                # Parse and display response
                try:
                    response_obj = json.loads(response_text)
                    if "response" in response_obj:
                        print(response_obj["response"])
                    else:
                        print(response_text)
                except:
                    print(response_text)
                
                print("=" * 70)
                print()
                print("✓ Test completed successfully!")
                
            except asyncio.TimeoutError:
                print("  ⚠ Timeout waiting for response (120 seconds)")
                return 1
                
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()
        return 1
    
    return 0

if __name__ == "__main__":
    try:
        exit_code = asyncio.run(test_inference())
        sys.exit(exit_code)
    except KeyboardInterrupt:
        print("\n\nInterrupted by user")
        sys.exit(1)

