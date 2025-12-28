#!/usr/bin/env python3
"""
Proper GGUF file splitter that respects tensor boundaries
Splits a GGUF file into N shards for distributed inference

Usage: python split_gguf_proper.py <input.gguf> <num_shards> [output_dir]
"""

import struct
import sys
import os
from pathlib import Path

# GGUF file format constants
GGUF_MAGIC = b'GGUF'
GGUF_VERSION = 3

def read_string(f):
    """Read a length-prefixed string from GGUF file"""
    length_bytes = f.read(8)
    if len(length_bytes) < 8:
        raise ValueError("Unexpected end of file while reading string length")
    length = struct.unpack('<Q', length_bytes)[0]
    # Safety check: prevent reading unreasonably large strings
    if length > 1024 * 1024:  # 1MB max string length
        raise ValueError(f"String length too large: {length} bytes (at position {f.tell() - 8})")
    data = f.read(length)
    if len(data) < length:
        raise ValueError(f"Unexpected end of file: expected {length} bytes, got {len(data)}")
    return data.decode('utf-8')

def read_metadata(f):
    """Read GGUF metadata section"""
    metadata = {}
    
    # Read number of key-value pairs
    num_kv = struct.unpack('<Q', f.read(8))[0]
    
    # Safety check: prevent reading unreasonably large metadata
    if num_kv > 10000:  # Sanity check
        raise ValueError(f"Too many metadata entries: {num_kv}")
    
    for _ in range(num_kv):
        key = read_string(f)
        value_type = struct.unpack('<I', f.read(4))[0]
        
        if value_type == 8:  # STRING
            value = read_string(f)
        elif value_type == 0:  # UINT8
            value = struct.unpack('<B', f.read(1))[0]
        elif value_type == 1:  # INT8
            value = struct.unpack('<b', f.read(1))[0]
        elif value_type == 2:  # UINT16
            value = struct.unpack('<H', f.read(2))[0]
        elif value_type == 3:  # INT16
            value = struct.unpack('<h', f.read(2))[0]
        elif value_type == 4:  # UINT32
            value = struct.unpack('<I', f.read(4))[0]
        elif value_type == 5:  # INT32
            value = struct.unpack('<i', f.read(4))[0]
        elif value_type == 6:  # FLOAT32
            value = struct.unpack('<f', f.read(4))[0]
        elif value_type == 7:  # BOOL
            value = struct.unpack('<?', f.read(1))[0]
        elif value_type == 9:  # ARRAY
            array_type = struct.unpack('<I', f.read(4))[0]
            array_len = struct.unpack('<Q', f.read(8))[0]
            # Safety check for array length
            if array_len > 1000000:  # 1M max array elements
                raise ValueError(f"Array length too large: {array_len}")
            value = []
            for _ in range(array_len):
                if array_type == 8:  # STRING
                    value.append(read_string(f))
                elif array_type == 4:  # UINT32
                    value.append(struct.unpack('<I', f.read(4))[0])
                elif array_type == 5:  # INT32
                    value.append(struct.unpack('<i', f.read(4))[0])
                elif array_type == 6:  # FLOAT32
                    value.append(struct.unpack('<f', f.read(4))[0])
                else:
                    # Skip unknown array types
                    f.read(4 * array_len)
        else:
            # Skip unknown types
            continue
            
        metadata[key] = value
    
    return metadata

def read_tensor_info(f):
    """Read tensor information"""
    name = read_string(f)
    n_dims = struct.unpack('<I', f.read(4))[0]
    dims = [struct.unpack('<Q', f.read(8))[0] for _ in range(n_dims)]
    tensor_type = struct.unpack('<I', f.read(4))[0]
    offset = struct.unpack('<Q', f.read(8))[0]
    
    return {
        'name': name,
        'dims': dims,
        'type': tensor_type,
        'offset': offset
    }

def get_tensor_size(dims, tensor_type):
    """Calculate tensor size in bytes"""
    # Tensor type sizes (simplified - actual GGUF has more types)
    type_sizes = {
        0: 1,   # F32
        1: 2,   # F16
        2: 4,   # Q4_0
        3: 4,   # Q4_1
        6: 4,   # Q5_0
        7: 4,   # Q5_1
        8: 4,   # Q8_0
        9: 1,   # Q8_1
        10: 1,  # Q2_K
        11: 1,  # Q3_K
        12: 1,  # Q4_K
        13: 1,  # Q5_K
        14: 1,  # Q6_K
    }
    
    element_size = type_sizes.get(tensor_type, 4)
    total_elements = 1
    for dim in dims:
        total_elements *= dim
    
    return total_elements * element_size

def split_gguf(input_file, num_shards, output_dir):
    """Split GGUF file into shards respecting tensor boundaries"""
    
    print(f"\nGGUF Proper Splitter")
    print(f"Input: {input_file}")
    print(f"Shards: {num_shards}")
    print(f"Output: {output_dir}\n")
    
    os.makedirs(output_dir, exist_ok=True)
    
    with open(input_file, 'rb') as f:
        # Read magic and version
        magic = f.read(4)
        if magic != GGUF_MAGIC:
            print(f"Error: Not a valid GGUF file (magic: {magic})")
            return False
        
        version = struct.unpack('<I', f.read(4))[0]
        print(f"GGUF version: {version}")
        
        # Read general metadata count
        general_metadata_count = struct.unpack('<Q', f.read(8))[0]
        print(f"General metadata count: {general_metadata_count}")
        
        # Read tensor metadata count (GGUF v3 has separate tensor metadata)
        tensor_metadata_count = struct.unpack('<Q', f.read(8))[0]
        print(f"Tensor metadata count: {tensor_metadata_count}")
        
        # Read general metadata
        print("Reading general metadata...")
        metadata = {}
        for i in range(general_metadata_count):
            if i % 50 == 0:
                print(f"  Reading metadata entry {i}/{general_metadata_count} (position: {f.tell()})")
            key = read_string(f)
            value_type = struct.unpack('<I', f.read(4))[0]
            
            if value_type == 8:  # STRING
                value = read_string(f)
            elif value_type == 0:  # UINT8
                value = struct.unpack('<B', f.read(1))[0]
            elif value_type == 1:  # INT8
                value = struct.unpack('<b', f.read(1))[0]
            elif value_type == 2:  # UINT16
                value = struct.unpack('<H', f.read(2))[0]
            elif value_type == 3:  # INT16
                value = struct.unpack('<h', f.read(2))[0]
            elif value_type == 4:  # UINT32
                value = struct.unpack('<I', f.read(4))[0]
            elif value_type == 5:  # INT32
                value = struct.unpack('<i', f.read(4))[0]
            elif value_type == 6:  # FLOAT32
                value = struct.unpack('<f', f.read(4))[0]
            elif value_type == 7:  # BOOL
                value = struct.unpack('<?', f.read(1))[0]
            elif value_type == 9:  # ARRAY
                array_type = struct.unpack('<I', f.read(4))[0]
                array_len = struct.unpack('<Q', f.read(8))[0]
                if array_len > 1000000:
                    raise ValueError(f"Array length too large: {array_len}")
                value = []
                for _ in range(array_len):
                    if array_type == 8:  # STRING
                        value.append(read_string(f))
                    elif array_type == 4:  # UINT32
                        value.append(struct.unpack('<I', f.read(4))[0])
                    elif array_type == 5:  # INT32
                        value.append(struct.unpack('<i', f.read(4))[0])
                    elif array_type == 6:  # FLOAT32
                        value.append(struct.unpack('<f', f.read(4))[0])
                    else:
                        f.read(4)  # Skip unknown array types
            else:
                # Skip unknown types - try to skip 4 bytes as a guess
                try:
                    f.read(4)
                except:
                    pass
                continue  # Don't add to metadata
                
            metadata[key] = value
        
        print(f"Found {len(metadata)} general metadata entries")
        
        # Skip tensor metadata (we don't need it for splitting)
        print(f"Skipping {tensor_metadata_count} tensor metadata entries...")
        for _ in range(tensor_metadata_count):
            _ = read_string(f)  # key
            value_type = struct.unpack('<I', f.read(4))[0]
            if value_type == 8:  # STRING
                _ = read_string(f)
            elif value_type in (0, 1, 7):  # UINT8, INT8, BOOL
                f.read(1)
            elif value_type in (2, 3):  # UINT16, INT16
                f.read(2)
            elif value_type in (4, 5, 6):  # UINT32, INT32, FLOAT32
                f.read(4)
            elif value_type == 9:  # ARRAY
                array_type = struct.unpack('<I', f.read(4))[0]
                array_len = struct.unpack('<Q', f.read(8))[0]
                for _ in range(array_len):
                    if array_type == 8:
                        _ = read_string(f)
                    elif array_type in (4, 5, 6):
                        f.read(4)
                    else:
                        f.read(4)
        
        # Read tensor count
        num_tensors = struct.unpack('<Q', f.read(8))[0]
        print(f"Found {num_tensors} tensors")
        
        # Read all tensor info
        tensors = []
        for i in range(num_tensors):
            tensor_info = read_tensor_info(f)
            tensor_info['size'] = get_tensor_size(tensor_info['dims'], tensor_info['type'])
            tensors.append(tensor_info)
            if i < 5:  # Show first 5
                print(f"  Tensor {i}: {tensor_info['name']} at offset {tensor_info['offset']} ({tensor_info['size']} bytes)")
        
        if num_tensors > 5:
            print(f"  ... and {num_tensors - 5} more tensors")
        
        # Get file size
        f.seek(0, 2)  # Seek to end
        file_size = f.tell()
        
        # Calculate tensor data start (current position after reading all tensor info)
        data_start = f.tell()
        
        print(f"\nTensor data starts at offset: {data_start}")
        print(f"Total file size: {file_size} bytes ({file_size / (1024**3):.2f} GB)")
        
        # Strategy: Split tensors across shards
        # Each shard gets a subset of tensors
        tensors_per_shard = num_tensors // num_shards
        remainder = num_tensors % num_shards
        
        print(f"\nSplitting {num_tensors} tensors across {num_shards} shards...")
        print(f"Tensors per shard: ~{tensors_per_shard} (with {remainder} extra tensors)\n")
        
        # Create shard files
        tensor_idx = 0
        for shard_num in range(num_shards):
            shard_path = os.path.join(output_dir, f"shard-{shard_num}.gguf")
            print(f"Creating shard {shard_num + 1}/{num_shards}: {os.path.basename(shard_path)}")
            
            with open(shard_path, 'wb') as shard_f:
                # Write GGUF header
                shard_f.write(GGUF_MAGIC)
                shard_f.write(struct.pack('<I', version))
                
                # Write metadata (copy from original)
                # For simplicity, we'll write a minimal metadata
                # In production, you'd copy the relevant metadata
                shard_f.write(struct.pack('<Q', 0))  # num_kv = 0 for now
                
                # Calculate how many tensors this shard gets
                shard_tensor_count = tensors_per_shard + (1 if shard_num < remainder else 0)
                
                # Write tensor count
                shard_f.write(struct.pack('<Q', shard_tensor_count))
                
                # Write tensor info and copy tensor data
                shard_data_offset = data_start
                for i in range(shard_tensor_count):
                    if tensor_idx >= num_tensors:
                        break
                    
                    tensor = tensors[tensor_idx]
                    
                    # Write tensor info
                    name_bytes = tensor['name'].encode('utf-8')
                    shard_f.write(struct.pack('<Q', len(name_bytes)))
                    shard_f.write(name_bytes)
                    shard_f.write(struct.pack('<I', len(tensor['dims'])))
                    for dim in tensor['dims']:
                        shard_f.write(struct.pack('<Q', dim))
                    shard_f.write(struct.pack('<I', tensor['type']))
                    shard_f.write(struct.pack('<Q', shard_data_offset))
                    
                    # Copy tensor data
                    f.seek(tensor['offset'])
                    tensor_data = f.read(tensor['size'])
                    shard_f.write(tensor_data)
                    shard_data_offset += len(tensor_data)
                    
                    tensor_idx += 1
            
            shard_size = os.path.getsize(shard_path)
            print(f"  Complete! ({shard_size / (1024**2):.2f} MB)\n")
    
    print("Shard splitting complete!")
    return True

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python split_gguf_proper.py <input.gguf> <num_shards> [output_dir]")
        print("Example: python split_gguf_proper.py model.gguf 8 models_cache/shard")
        sys.exit(1)
    
    input_file = sys.argv[1]
    num_shards = int(sys.argv[2])
    output_dir = sys.argv[3] if len(sys.argv) > 3 else "models_cache/shards"
    
    if not os.path.exists(input_file):
        print(f"Error: File not found: {input_file}")
        sys.exit(1)
    
    split_gguf(input_file, num_shards, output_dir)

