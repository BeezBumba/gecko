import json
import zipfile
from pathlib import Path

OUTPUT_PATH = Path(__file__).with_name("game_index.json")
OUTPUT_TXT_PATH = Path(__file__).with_name("game_index.txt")

ROMS_PATH = Path("\\\\VIBRATOR\\Roms\\Nintendo - GameCube")

WHITELIST = ["(USA)"]
BLACKLIST = [
    "Alpha", "Beta", "Proto", "Rev ", "Action Replay", "(v1.", "(Unl)", "Disc 2", "Disc 3", "Demo", "Preview", "Bonus", "Debug"
]

RVZ_MAGIC = b"RVZ\x01"
DISC_HEADER_OFFSET = 0x48 + 0x10
DISC_HEADER_SIZE = 0x80


def is_allowed(name: str) -> bool:
    lower = name.lower()
    if not any(w.lower() in lower for w in WHITELIST):
        return False
    return not any(b.lower() in lower for b in BLACKLIST)


def read_disc_header(fp) -> bytes:
    magic = fp.read(4)
    assert magic == RVZ_MAGIC, f"bad RVZ magic: {magic!r}"
    fp.seek(DISC_HEADER_OFFSET)
    return fp.read(DISC_HEADER_SIZE)


def parse_header(header: bytes) -> tuple[str, str]:
    game_code = header[:4].decode("ascii", errors="replace")
    name_bytes = header[0x20:DISC_HEADER_SIZE].split(b"\x00", 1)[0]
    internal_name = name_bytes.decode("utf-8", errors="replace").strip()
    return game_code, internal_name


def index_zip(zip_path: Path) -> tuple[str, str, str] | None:
    with zipfile.ZipFile(zip_path) as zf:
        rvz_entries = [n for n in zf.namelist() if n.lower().endswith(".rvz")]
        if not rvz_entries:
            return None
        with zf.open(rvz_entries[0]) as fp:
            header = read_disc_header(fp)
    game_code, internal_name = parse_header(header)
    return game_code, internal_name, zip_path.stem


def main() -> None:
    collisions: dict[str, list[tuple[str, str]]] = {}
    for zip_path in sorted(ROMS_PATH.glob("*.zip")):
        if not is_allowed(zip_path.name):
            continue
        try:
            entry = index_zip(zip_path)
        except Exception as e:
            print(f"[skip] {zip_path.name}: {e}")
            continue
        if entry is None:
            continue
        game_code, internal_name, stem = entry
        collisions.setdefault(game_code, []).append((internal_name, stem))
        print(f"{game_code}  {internal_name}  ({stem})")

    dupes = {code: entries for code, entries in collisions.items() if len(entries) > 1}
    print(f"\nindexed {len(collisions)} unique game codes across {sum(len(v) for v in collisions.values())} zips")
    if dupes:
        print(f"\n{len(dupes)} duplicate game code(s):")
        for code, entries in sorted(dupes.items()):
            print(f"  {code}:")
            for internal_name, stem in entries:
                print(f"    - {internal_name}  ({stem})")
    else:
        print("\nno duplicate game codes")

    index = {
        code: [{"internal_name": name, "filename": stem} for name, stem in entries]
        for code, entries in sorted(collisions.items())
    }
    OUTPUT_PATH.write_text(json.dumps(index, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"\nwrote {OUTPUT_PATH}")

    lines = [f"{code}={entries[0][1]}" for code, entries in sorted(collisions.items())]
    OUTPUT_TXT_PATH.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUTPUT_TXT_PATH}")


if __name__ == "__main__":
    main()
