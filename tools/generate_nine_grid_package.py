"""Derive a tiled nine-grid package from the unchanged official Basics export.

Resource IDs/geometry/atlas files stay intact. Only package identity, URL IDs,
dependency metadata and native nonzero tileGridIndice flags change.
"""
import struct
from generate_popup_packages import Package, read


def main():
    package = Package("nine0001", "NineGrid")
    data, pos = package.blocks[1], 2
    changed = 0
    for _ in range(read("H", data, 0)):
        end = pos + 4 + read("I", data, pos)
        start = pos + 4
        if data[start] == 0 and data[start + 18] == 1:
            struct.pack_into(">i", data, start + 35, 1)
            changed += 1
        pos = end
    assert changed > 0
    package.write("NineGrid.fui", ("9leh0eyf", "Basics"))
    print(f"Enabled native tiled nine-grid on {changed} images")


if __name__ == "__main__":
    main()
