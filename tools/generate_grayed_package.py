"""Derive serialized-gray instances from native Basics, retaining its controls.

Only package identity/URL IDs, dependency metadata and instance grayed flags
change. All controllers, gears, selected states, component records and atlases
otherwise remain the original exported data.
"""
from generate_popup_packages import Package, indexed, read


def grayed_offset(data, properties):
    pos = properties + 17  # type, source/package IDs, id/name, x/y
    for size in [8, 16, 8, 8, 9]:  # size, limits, scale, skew, pivot
        present = data[pos]
        assert present in [0, 1]
        pos += 1 + (size if present else 0)
    return pos + 10  # alpha, rotation, visible, touchable


def main():
    package = Package("gray0001", "Grayed")
    data = package.blocks[1]
    changed = 0
    for component in ["Demo_Component", "Demo_Button"]:
        children = indexed(data, package.components[component], 2)
        pos = children + 2
        for _ in range(read("H", data, children)):
            child = pos + 2
            properties = indexed(data, child, 0)
            # Only referenced components, preserving native leaf/font data.
            if data[properties] == 9 and read("H", data, properties + 1) < 65533:
                offset = grayed_offset(data, properties)
                assert data[offset] in [0, 1]
                if data[offset] == 0:
                    data[offset] = 1
                    changed += 1
            pos = child + read("H", data, pos)
    assert changed > 0
    package.write("Grayed.fui", ("9leh0eyf", "Basics"))
    print(f"Enabled serialized grayed on {changed} native component instances")


if __name__ == "__main__":
    main()
