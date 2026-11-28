#!/bin/bash
echo 'Creating icon...'
python3 -c "
import struct, zlib

def create_png(width, height, filename):
    def crc32(data):
        return zlib.crc32(data) & 0xffffffff
    
    def chunk(type, data):
        length = len(data)
        crc = crc32(type + data)
        return struct.pack('>I', length) + type + data + struct.pack('>I', crc)
    
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    ihdr = chunk(b'IHDR', ihdr_data)
    
    raw_data = b'\x00' * (width * height * 4 + height)
    compressed = zlib.compress(raw_data, 9)
    idat = chunk(b'IDAT', compressed)
    
    iend = chunk(b'IEND', b'')
    
    with open(filename, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n')
        f.write(ihdr)
        f.write(idat)
        f.write(iend)

create_png(256, 256, '/Users/ntars/Documents/Github/Relax/src-tauri/icons/icon.png')
print('Icon created successfully!')
"

echo 'Done!'
