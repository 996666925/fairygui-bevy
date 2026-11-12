"""Derive deterministic dependency fixtures from the unmodified Basics export.

Only package identities, URL strings and selected source-package references
change. Component instance data, images, controllers and relations stay native.
Run from any directory; no external dependencies are required.
"""
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "crates/fairygui-core/tests/fixtures"


def read(fmt, data, pos):
    return struct.unpack_from(">" + fmt, data, pos)[0]


def utf(text):
    data = text.encode("utf-8")
    return struct.pack(">H", len(data)) + data


def indexed(data, start, block):
    count, short = data[start:start + 2]
    assert block < count
    offset = read("H" if short else "I", data, start + 2 + block * (2 if short else 4))
    return start + offset if offset else None


class Package:
    def __init__(self, package_id, name):
        raw = (FIXTURES / "Basics.fui").read_bytes()
        self.version = read("I", raw, 4)
        pos = 9
        for _ in range(2):
            pos += 2 + read("H", raw, pos)
        body = raw[pos + 20:]
        if raw[8]:
            try:
                body = zlib.decompress(body)
            except zlib.error:
                body = zlib.decompress(body, -15)
        self.package_id, self.name = package_id, name
        starts = [indexed(body, 0, i) for i in range(body[0])]
        self.blocks = [bytearray(body[start:min((p for p in starts if p and p > start), default=len(body))]) if start else bytearray() for start in starts]
        self.strings = []
        pos = 4
        for _ in range(read("I", self.blocks[4], 0)):
            size = read("H", self.blocks[4], pos)
            self.strings.append(bytes(self.blocks[4][pos + 2:pos + 2 + size]).decode("utf-8"))
            pos += 2 + size
        self.strings = [s.replace("ui://9leh0eyf", "ui://" + package_id) for s in self.strings]
        self.components = {}
        self.component_ids = {}
        data, pos = self.blocks[1], 2
        for _ in range(read("H", data, 0)):
            end = pos + 4 + read("I", data, pos)
            start = pos + 4
            if data[start] == 3:
                name = self.strings[read("H", data, start + 3)]
                self.components[name] = start + 23
                self.component_ids[name] = self.strings[read("H", data, start + 1)]
            pos = end

    def string(self, value):
        if value not in self.strings:
            self.strings.append(value)
        return self.strings.index(value)

    def replace_url(self, resource, target):
        original = "ui://" + self.package_id + resource
        assert original in self.strings, original
        self.strings = [target if s == original else s for s in self.strings]

    def foreign_child(self, component, child_index, package_id, resource=None):
        data = self.blocks[1]
        start = self.components[component]
        pos = indexed(data, start, 2) + 2
        for _ in range(child_index):
            pos += 2 + read("H", data, pos)
        child = pos + 2
        properties = indexed(data, child, 0)
        struct.pack_into(">H", data, properties + 3, self.string(package_id))
        if resource is not None:
            struct.pack_into(">H", data, properties + 1, self.string(resource))

    def write(self, filename, dependency):
        self.blocks[0] = bytearray(struct.pack(">hHHh", 1, self.string(dependency[0]), self.string(dependency[1]), 0))
        self.blocks[4] = bytearray(struct.pack(">I", len(self.strings)) + b"".join(utf(s) for s in self.strings))
        header_size = 2 + len(self.blocks) * 4
        offsets, offset = [], header_size
        for block in self.blocks:
            offsets.append(offset if block else 0)
            offset += len(block)
        body = bytes([len(self.blocks), 0]) + b"".join(struct.pack(">I", p) for p in offsets) + b"".join(self.blocks)
        compressor = zlib.compressobj(level=9, wbits=-15)
        output = b"FGUI" + struct.pack(">IB", self.version, 1) + utf(self.package_id) + utf(self.name) + bytes(20) + compressor.compress(body) + compressor.flush()
        (FIXTURES / filename).write_bytes(output)
        print(f"{filename}: {len(output)} bytes")


def main():
    host, theme = Package("host0001", "PopupHost"), Package("theme001", "PopupTheme")
    # Native component IDs stay unchanged in both packages, deliberately causing
    # ID collisions to catch any accidental local-package fallback.
    host.replace_url("rt103y", "ui://PopupTheme/ComboBoxPopup")
    theme.replace_url("rt1040", "ui://PopupHost/PopupMenuItem")
    theme.foreign_child("PopupMenu", 0, "host0001")
    # A real component reference (not merely an image) also crosses packages.
    theme.foreign_child("Demo_Popup", 0, "host0001")
    host.write("PopupHost.fui", ("theme001", "PopupTheme"))
    theme.write("PopupTheme.fui", ("host0001", "PopupHost"))
    cycle = Package("cycle001", "PopupCycle")
    cycle.foreign_child("Demo_Popup", 1, "cycle001", cycle.component_ids["Demo_Popup"])
    cycle.write("PopupCycle.fui", ("host0001", "PopupHost"))


if __name__ == "__main__":
    main()
