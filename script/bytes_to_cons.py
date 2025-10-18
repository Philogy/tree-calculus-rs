import sys

byte_arg = sys.argv[1]
if byte_arg.startswith('0x'):
    byte_arg = bytes.fromhex(byte_arg.removeprefix('0x'))
else:
    byte_arg = int(byte_arg).to_bytes(32, 'big')

nibbles = [f'{b:02x}' for b in byte_arg]

l = ''
for i in range(len(nibbles)):
    nib = nibbles.pop()
    if i == 0:
        l = f'cons hex_{nib} nil'
    else:
        l = f'cons hex_{nib} ({l})'

print(l)


