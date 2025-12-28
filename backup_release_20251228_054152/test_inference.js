// Test inference request via WebSocket
const WebSocket = require('ws');

const question = "How are a cat and a snake related?";
const wsUrl = "ws://localhost:8081";

console.log("\n========================================");
console.log("  TESTING INFERENCE VIA WEBSOCKET");
console.log("========================================\n");
console.log(`Question: ${question}\n`);

const ws = new WebSocket(wsUrl);

ws.on('open', function open() {
    console.log("[1/5] ✓ Connected to WebSocket server");
    
    // Create query request
    const queryRequest = {
        query: question,
        request_id: `test-${Date.now()}`
    };
    
    console.log("\n[2/5] Sending inference request...");
    console.log(`  Request: ${JSON.stringify(queryRequest)}`);
    
    ws.send(JSON.stringify(queryRequest));
    console.log("  ✓ Request sent");
    
    console.log("\n[3/5] Waiting for response...");
});

let messagesReceived = 0;
let finalResponse = null;

ws.on('message', function message(data) {
    messagesReceived++;
    
    try {
        const msg = JSON.parse(data.toString());
        
        // Log all messages for debugging
        console.log(`\n[4/5] Received message #${messagesReceived}:`);
        console.log(`  Type: ${msg.message_type || 'query_response'}`);
        
        // Check if this is the final query response
        if (msg.response !== undefined) {
            finalResponse = msg;
            console.log("\n[5/5] ✓✓✓ INFERENCE RESPONSE RECEIVED ✓✓✓");
            console.log("\n========================================");
            console.log("  INFERENCE RESULT");
            console.log("========================================\n");
            console.log("Response:");
            console.log(msg.response);
            console.log("\n----------------------------------------");
            console.log(`Tokens Generated: ${msg.tokens}`);
            console.log(`Latency: ${msg.latency_ms}ms`);
            console.log(`Success: ${msg.success}`);
            if (msg.shards_used && msg.shards_used.length > 0) {
                console.log(`Shards Used: ${msg.shards_used.length}`);
                msg.shards_used.forEach((shard, i) => {
                    console.log(`  - Shard ${shard.shard_id}: ${shard.latency_ms}ms`);
                });
            }
            console.log("========================================\n");
            
            // Close connection after receiving response
            setTimeout(() => {
                ws.close();
                process.exit(0);
            }, 1000);
        } else if (msg.message_type === 'pipeline_status') {
            console.log(`  Pipeline Status: ${msg.online_nodes}/${msg.total_nodes} nodes online`);
            console.log(`  Complete: ${msg.is_complete}`);
            if (msg.missing_shards && msg.missing_shards.length > 0) {
                console.log(`  Missing Shards: ${msg.missing_shards.join(', ')}`);
            }
        } else if (msg.message_type === 'pipeline_update') {
            console.log(`  Update: ${msg.stage} - ${msg.status}`);
            if (msg.shard_id !== undefined) {
                console.log(`  Shard: ${msg.shard_id}`);
            }
        } else {
            console.log(`  Data: ${JSON.stringify(msg).substring(0, 200)}...`);
        }
    } catch (e) {
        console.log(`  Raw message: ${data.toString().substring(0, 200)}...`);
    }
    
    // Timeout after 60 seconds if no response
    if (messagesReceived === 1) {
        setTimeout(() => {
            if (!finalResponse) {
                console.log("\n⚠️  Timeout waiting for inference response (60s)");
                console.log("   Received status messages but no final response");
                ws.close();
                process.exit(1);
            }
        }, 60000);
    }
});

ws.on('error', function error(err) {
    console.error("\n❌ WebSocket Error:", err.message);
    process.exit(1);
});

ws.on('close', function close() {
    if (!finalResponse) {
        console.log("\n⚠️  Connection closed before receiving response");
        process.exit(1);
    }
});

// Overall timeout
setTimeout(() => {
    if (!finalResponse) {
        console.log("\n❌ Timeout: No response received after 90 seconds");
        ws.close();
        process.exit(1);
    }
}, 90000);

