"""Derive bitmap text layout samples using native Basics fonts and atlases.

Identity/URL IDs, font auto-scale headers and selected text instance properties
change. Font glyph/texture records and atlas pixels remain native.
"""
import struct
from generate_popup_packages import Package, indexed, read
from generate_grayed_package import grayed_offset


def main():
    package = Package("bmfont01", "BitmapText")
    data, pos = package.blocks[1], 2
    fonts = {}
    for _ in range(read("H", data, 0)):
        end = pos + 4 + read("I", data, pos)
        start = pos + 4
        if data[start] == 5:
            name = package.strings[read("H", data, start + 3)]
            fonts[name] = package.strings[read("H", data, start + 1)]
            header = indexed(data, start + 22, 0)
            data[header + 2] = 1  # autoScaleSize
            if name == "HitNumber":
                struct.pack_into(">i", data, header + 4, 48)
                struct.pack_into(">i", data, header + 12, 64)
        pos = end
    names = ["n0", "n2", "n3", "n4", "n7", "n8", "n10", "n11", "n17"]
    children = indexed(data, package.components["Demo_Text"], 2)
    pos = children + 2
    for _ in range(read("H", data, children)):
        child = pos + 2
        properties = indexed(data, child, 0)
        name = package.strings[read("H", data, properties + 7)]
        visible = grayed_offset(data, properties) - 2
        data[visible] = int(name in names)
        if name in names:
            i = names.index(name)
            # Two families: native image glyphs (even) and atlas-font glyphs.
            family = "HitNumber" if i % 2 == 0 else "BMFontTest"
            url = "ui://" + package.package_id + fonts[family]
            style = indexed(data, child, 5)
            struct.pack_into(">Hh", data, style, package.string(url), [24, 32, 48][i % 3])
            data[style + 8] = i % 3  # align
            data[style + 9] = i % 3  # vertical align
            struct.pack_into(">hh", data, style + 10, 3, 0)
            data[style + 14] = 0  # UBB
            data[style + 15] = [0, 1, 2, 3, 4, 0, 1, 2, 0][i]
            data[style + 16:style + 20] = bytes([0, 0, 0, int(i == 4)])
            struct.pack_into(">ii", data, properties + 9, 35 + (i % 3) * 340, 90 + (i // 3) * 150)
            assert data[properties + 17] == 1
            struct.pack_into(">ii", data, properties + 18, [245, 210, 270][i % 3], 110)
            content = "0123?45\n67890" if i % 2 == 0 else "使用嵌入字体\nUI#"
            text = indexed(data, child, 6)
            struct.pack_into(">H", data, text, package.string(content))
        pos = child + read("H", data, pos)
    package.write("BitmapText.fui", ("9leh0eyf", "Basics"))
    print("Enabled scalable native image/atlas fonts and nine layout sample fields")


if __name__ == "__main__":
    main()
