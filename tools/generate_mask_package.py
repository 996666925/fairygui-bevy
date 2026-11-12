"""Add native block-4 mask/reversed/hit fields to otherwise native components."""
import struct
from generate_popup_packages import Package, indexed, read


def main():
    package = Package('mask0001', 'Masks')
    source = package.blocks[1]
    records = []
    pos = 2
    changed = 0
    for _ in range(read('H', source, 0)):
        size = read('I', source, pos)
        item = bytes(source[pos+4:pos+4+size])
        if item[0] == 3 and package.strings[read('H', item, 3)] in ['Demo_Graph', 'Demo_Image']:
            name = package.strings[read('H', item, 3)]
            raw_size = read('I', item, 19)
            raw = item[23:23+raw_size]
            starts = [indexed(raw, 0, i) for i in range(raw[0])]
            blocks = [raw[start:min((p for p in starts if p and p > start), default=len(raw))] if start else b'' for start in starts]
            original = blocks[4]
            assert read('h', original, 3) == -1
            blocks[4] = original[:3] + struct.pack('>hBHii', 0, int(name == 'Demo_Image'), package.string('t9fj54'), 17, -23) + original[15:]
            header_size = 2 + len(blocks)*4
            offsets = []
            offset = header_size
            for block in blocks:
                offsets.append(offset if block else 0)
                offset += len(block)
            rebuilt = bytes([len(blocks), 0]) + b''.join(struct.pack('>I',p) for p in offsets) + b''.join(blocks)
            item = item[:19] + struct.pack('>I', len(rebuilt)) + rebuilt + item[23+raw_size:]
            changed += 1
        records.append(struct.pack('>I',len(item))+item)
        pos += 4+size
    assert changed == 2
    package.blocks[1] = bytearray(struct.pack('>H',len(records)) + b''.join(records))
    package.write('Masks.fui', ('9leh0eyf','Basics'))
    print('2 native component records: normal/reversed mask plus explicit Pixel hit offsets')


if __name__ == '__main__':
    main()
