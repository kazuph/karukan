#!/usr/bin/env python3
"""gtype feedback を karukan ユーザー辞書用 TSV に同期する。

優先順:
1. API: http://127.0.0.1:3210/api/feedback (timeout 2s)
2. フォールバック TSV: ~/Library/Application Support/GtypeMac/karukan_feedback.tsv

API応答:
  [{reading, wrong, correct, source, timestamp, id}, ...]

フォールバック TSV:
  reading<TAB>wrong<TAB>correct<TAB>source<TAB>timestamp

※ manual 行は reading が空の代わりに wrong 列に読みが入っている。
出力: reading<TAB>correct<TAB>名詞<TAB>gtype feedback
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path
from typing import Iterable
from urllib.request import Request, urlopen

API_URL = "http://127.0.0.1:3210/api/feedback"
API_TIMEOUT_SECONDS = 2
FEEDBACK_SOURCE_PATH = Path("~/Library/Application Support/GtypeMac/karukan_feedback.tsv")
DEFAULT_COMMENT = "gtype feedback"


def _is_kana(text: str) -> bool:
    if not text:
        return False
    for ch in text:
        code = ord(ch)
        if (
            0x3040 <= code <= 0x309F  # ひらがな
            or 0x30A0 <= code <= 0x30FF  # カタカナ
            or ch == "ー"
        ):
            continue
        return False
    return True


def _normalize_entry(
    reading: str, wrong: str, correct: str
) -> tuple[str, str] | None:
    reading = (reading or "").strip()
    if not reading:
        reading = (wrong or "").strip()
    correct = (correct or "").strip()
    if not reading or not correct:
        return None
    if not _is_kana(reading):
        return None
    return reading, correct


def _collect_api_entries() -> tuple[list[tuple[str, str]], int] | None:
    request = Request(API_URL, headers={"Accept": "application/json"})
    try:
        with urlopen(request, timeout=API_TIMEOUT_SECONDS) as response:
            if response.status != 200:
                print(
                    f"gtype API returned status={response.status}",
                    file=sys.stderr,
                )
                return None
            payload = json.loads(response.read().decode("utf-8"))
    except Exception as e:
        print(f"gtype API fetch failed: {type(e).__name__}: {e}", file=sys.stderr)
        return None

    if not isinstance(payload, list):
        print(
            f"unexpected API payload type: {type(payload).__name__}",
            file=sys.stderr,
        )
        return None

    entries: list[tuple[str, str]] = []
    skipped = 0
    for row in payload:
        if not isinstance(row, dict):
            skipped += 1
            continue
        reading = row.get("reading", "") or ""
        wrong = row.get("wrong", "") or ""
        correct = row.get("correct", "") or ""
        normalized = _normalize_entry(str(reading), str(wrong), str(correct))
        if not normalized:
            skipped += 1
            continue
        entries.append(normalized)
    return entries, skipped


def _collect_feedback_tsv_entries(path: Path) -> tuple[list[tuple[str, str]], int] | None:
    if not path.exists():
        print(f"gtype feedback file not found: {path}", file=sys.stderr)
        return None

    entries: list[tuple[str, str]] = []
    skipped = 0
    try:
        for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            if not line or line.startswith("#"):
                continue
            cols = line.split("\t")
            reading = (cols[0] if cols else "").strip()
            wrong = (cols[1] if len(cols) >= 2 else "").strip()
            correct = (cols[2] if len(cols) >= 3 else "").strip()
            normalized = _normalize_entry(reading, wrong, correct)
            if not normalized:
                skipped += 1
                continue
            entries.append(normalized)
    except OSError as e:
        print(f"failed to read {path}: {e}", file=sys.stderr)
        return None

    return entries, skipped


def _dedup_and_format(entries: Iterable[tuple[str, str]]) -> list[str]:
    seen = set()
    output = []
    for reading, surface in entries:
        key = f"{reading}\t{surface}"
        if key in seen:
            continue
        seen.add(key)
        output.append(f"{reading}\t{surface}\t名詞\t{DEFAULT_COMMENT}")
    return output


def _write_atomic(lines: list[str], output_path: Path) -> int:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w",
        dir=output_path.parent,
        delete=False,
        encoding="utf-8",
        prefix=".gtype-feedback-sync-",
        suffix=".tmp",
    ) as fp:
        fp.write("\n".join(lines))
        fp.write("\n" if lines else "")
        temp_name = fp.name

    temp_path = Path(temp_name)
    temp_path.replace(output_path)
    return len(lines)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Sync gtype feedback into karukan user dictionary TSV"
    )
    parser.add_argument(
        "output",
        help="Path to write karukan user dict TSV",
    )
    args = parser.parse_args()
    output_path = Path(args.output).expanduser()

    entries: list[tuple[str, str]] = []
    skipped = 0

    from_api = _collect_api_entries()
    if from_api is not None:
        records, skipped_api = from_api
        entries.extend(records)
        skipped += skipped_api
    else:
        from_file = _collect_feedback_tsv_entries(FEEDBACK_SOURCE_PATH.expanduser())
        if from_file is None:
            print("no feedback source is available", file=sys.stderr)
            return 1
        records, skipped_file = from_file
        entries.extend(records)
        skipped += skipped_file

    lines = _dedup_and_format(entries)
    _write_atomic(lines, output_path)
    print(
        f"synced {len(lines)} entries to {output_path} (skipped {skipped} invalid)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
