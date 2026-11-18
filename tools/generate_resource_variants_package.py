"""Derive branch/HD resource fixtures from the bundled Basics export.

Keep native component, glyph and frame records, changing resource identities,
branch tables, dimensions and timing so selecting the wrong variant is visible.
"""
import struct
from generate_popup_packages import Package, read, indexed, utf, FIXTURES


def generate():
    package = Package("variants", "ResourceVariants")
    data = package.blocks[1]
    entries, selected = [], {}
    pos = 2
    for _ in range(read("H", data, 0)):
        start = pos + 4
        end = start + read("I", data, pos)
        entry = bytearray(data[start:end])
        name_index = read("H", entry, 3)
        name = package.strings[name_index] if name_index < len(package.strings) else ""
        if (entry[0] in (0, 1, 5) and entry[0] not in selected) or name == "Demo_Popup":
            selected[entry[0]] = len(entries)
        entries.append(entry)
        pos = end

    sprites = []
    data, pos = package.blocks[2], 2
    for _ in range(read("H", data, 0)):
        start = pos + 2
        end = start + read("H", data, pos)
        sprites.append(bytes(data[start:end]))
        pos = end

    for kind, entry_index in selected.items():
        original = entries[entry_index]
        assert original[-4:] == b"\xff\xfe\0\0"
        base_id = package.strings[read("H", original, 1)]
        # Base -> night -> HD, with a deliberate hole in the second HD level.
        variants = [(base_id, "", 40, 20), (f"v{kind}night", "night", 60, 30),
                    (f"v{kind}hd", "", 120, 60)]
        for level, (item_id, branch, width, height) in enumerate(variants):
            entry = bytearray(original[:-4])
            struct.pack_into(">H", entry, 1, package.string(item_id))
            if level == 2:
                name = package.strings[read("H", original, 3)]
                struct.pack_into(">H", entry, 3, package.string(name + "@hd"))
            struct.pack_into(">ii", entry, 10, width, height)
            if kind == 3:
                header = indexed(entry, 23, 0)
                struct.pack_into(">ii", entry, header, width, height)
            elif kind in (1, 5):
                header = indexed(entry, 23 if kind == 1 else 22, 0)
                struct.pack_into(">i", entry, header + (4 if kind == 5 else 0),
                                 [24, 36, 72][level] if kind == 5 else [100, 150, 300][level])
                if kind == 5:
                    glyphs = indexed(entry, 22, 1)
                    image = entries[selected[0]]
                    struct.pack_into(">H", entry, glyphs + 8, read("H", image, 1))
            entry.extend(struct.pack(">H", package.string(branch) if branch else 65534))
            entry.extend(b"\x02" + struct.pack(">HH", 65534, package.string(variants[1][0]))
                         if level == 0 else b"\0")
            entry.extend(b"\x02" + struct.pack(">HH", package.string(variants[2][0]), 65534)
                         if level == 1 else b"\0")
            if level == 0:
                entries[entry_index] = entry
            else:
                entries.append(entry)
            if kind == 0 and level:
                original_sprite = next(s for s in sprites if read("H", s, 0) == package.string(base_id))
                # New untrimmed rectangles deliberately differ from the base.
                sprites.append(struct.pack(">HHiiiiBB", package.string(item_id),
                                           read("H", original_sprite, 2), 7 * level, 9 * level,
                                           width, height, 0, 0))

    package.blocks[1] = bytearray(struct.pack(">H", len(entries)) + b"".join(
        struct.pack(">I", len(entry)) + entry for entry in entries))
    package.blocks[2] = bytearray(struct.pack(">H", len(sprites)) + b"".join(
        struct.pack(">H", len(sprite)) + sprite for sprite in sprites))
    # Package.write normally emits no branches. Replace the block after its
    # dependency setup by writing the deterministic header/body here instead.
    package.blocks[0] = bytearray(struct.pack(">hhHH", 0, 2,
        package.string("day"), package.string("night")))
    package.blocks[4] = bytearray(struct.pack(">I", len(package.strings))
                                 + b"".join(utf(s) for s in package.strings))
    offset = 2 + len(package.blocks) * 4
    offsets = []
    for block in package.blocks:
        offsets.append(offset if block else 0)
        offset += len(block)
    body = bytes([len(package.blocks), 0]) + b"".join(struct.pack(">I", n) for n in offsets)
    body += b"".join(package.blocks)
    import zlib
    compressor = zlib.compressobj(level=9, wbits=-15)
    output = b"FGUI" + struct.pack(">IB", package.version, 1)
    output += utf(package.package_id) + utf(package.name) + bytes(20)
    output += compressor.compress(body) + compressor.flush()
    (FIXTURES / "ResourceVariants.fui").write_bytes(output)
    print(f"ResourceVariants.fui: {len(output)} bytes")
    host = Package("res-host", "ResourceHost")
    host.foreign_child("Demo_Popup", 0, package.package_id, package.component_ids["Demo_Popup"])
    host.write("ResourceHost.fui", (package.package_id, package.name))


if __name__ == "__main__":
    generate()
