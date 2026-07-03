#!/usr/bin/env python3
"""Google 日本語入力 (mozc) の user_dictionary.db を karukan の user_dicts 用 TSV に変換する。

使い方:
    python3 scripts/mozc_user_dictionary_db_to_tsv.py \
        "~/Library/Application Support/Google/JapaneseInput/user_dictionary.db" \
        google_ime_import.tsv

db は mozc の UserDictionaryStorage protobuf。karukan の TSV ローダは
「読み<TAB>表記」の2列しか見ないため、生の protobuf を辿って各エントリの
key(=読み, field 1) と value(=表記, field 2) の文字列だけを抽出する。
品詞 enum のマッピングに依存しないので mozc 側の proto 変更にも強い。

読みが `:` で始まるエントリ（絵文字パック等）は、karukan では `:` が
Emoji モードのトリガーで読みとして到達不能なため既定でスキップする。
"""

import sys
from pathlib import Path


def read_varint(buf: bytes, i: int):
    value = shift = 0
    while True:
        b = buf[i]
        i += 1
        value |= (b & 0x7F) << shift
        if not b & 0x80:
            return value, i
        shift += 7


def fields(buf: bytes):
    """protobuf メッセージを (field_no, wire_type, value) で列挙する。"""
    i = 0
    while i < len(buf):
        tag, i = read_varint(buf, i)
        fno, wt = tag >> 3, tag & 7
        if wt == 0:  # varint
            v, i = read_varint(buf, i)
        elif wt == 2:  # length-delimited
            ln, i = read_varint(buf, i)
            v = buf[i : i + ln]
            i += ln
        elif wt == 5:  # 32-bit
            v = buf[i : i + 4]
            i += 4
        elif wt == 1:  # 64-bit
            v = buf[i : i + 8]
            i += 8
        else:
            raise ValueError(f"unsupported wire type {wt}")
        yield fno, wt, v


def extract_entries(db_bytes: bytes):
    """(読み, 表記, 辞書名) を列挙する。"""
    for fno, wt, v in fields(db_bytes):
        if fno != 2 or wt != 2:  # UserDictionaryStorage.dictionaries
            continue
        name = ""
        for dfno, dwt, dv in fields(v):
            if dfno == 2 and dwt == 2:  # UserDictionary.name
                name = dv.decode("utf-8", "replace")
            elif dfno == 4 and dwt == 2:  # UserDictionary.entries
                key = value = None
                for efno, ewt, ev in fields(dv):
                    if efno == 1 and ewt == 2:
                        key = ev.decode("utf-8", "replace")
                    elif efno == 2 and ewt == 2:
                        value = ev.decode("utf-8", "replace")
                if key and value:
                    yield key, value, name


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(1)
    db_path = Path(sys.argv[1]).expanduser()
    out_path = Path(sys.argv[2]).expanduser()

    data = db_path.read_bytes()
    seen = set()
    rows = []
    skipped_colon = 0
    for key, value, name in extract_entries(data):
        if key.startswith(":"):
            skipped_colon += 1
            continue
        if (key, value) in seen:
            continue
        seen.add((key, value))
        comment = f"Google IME import ({name})" if name else "Google IME import"
        rows.append(f"{key}\t{value}\t名詞\t{comment}")

    with out_path.open("w", encoding="utf-8") as f:
        f.write("# karukan user dictionary (Mozc/Google IME TSV)\n")
        f.write("# reading\tsurface\tpart-of-speech\tcomment\n")
        f.write("\n".join(rows) + "\n")

    print(f"wrote {len(rows)} entries to {out_path}"
          f" (skipped {skipped_colon} ':'-prefixed emoji-pack entries)")


if __name__ == "__main__":
    main()
